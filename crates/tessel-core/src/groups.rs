//! Groupes MLS (story 02-04).
//!
//! Un groupe réunit des appareils, pas des comptes : ajouter quelqu'un, c'est ajouter
//! chacun de ses appareils ; un compte en a plusieurs. Chaque feuille qui entre ou
//! change doit porter un certificat d'appareil valide (story 02-02), sinon le commit
//! ou le *Welcome* est refusé.
//!
//! Le serveur ordonne les commits (story 04-02) : un seul gagne par epoch. Un commit
//! se prépare ([`Client::add_devices`], [`Client::remove_devices`]…), part au serveur,
//! puis le client apprend s'il est accepté ([`Client::commit_accepted`]) ou refusé
//! ([`Client::commit_rejected`]). Refusé, le client traite les commits qui l'ont
//! devancé, puis [`Client::retry_commit`] rejoue la même intention sur l'état à jour.
//!
//! Tout part chiffré (`PrivateMessage`), commits compris : le serveur ne voit que
//! l'identifiant du groupe, l'epoch et le type de message.

use std::fmt;

use openmls::prelude::{
    KeyPackage, LeafNodeIndex, Member, MlsGroup, MlsGroupCreateConfig, MlsGroupJoinConfig,
    MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, PURE_CIPHERTEXT_WIRE_FORMAT_POLICY,
    ProcessedMessageContent, Sender, StagedCommit, StagedWelcome,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use tls_codec::{Deserialize as _, Serialize as _};

use crate::client::Client;
use crate::identity::{AccountId, Device, DeviceCertificate, DeviceId, IdentityError};
use crate::key_packages::{VerifiedKeyPackage, capabilities};
use crate::suite::CIPHERSUITE;

/// Nombre d'epochs passées dont on garde les clés, pour lire un message chiffré juste
/// avant un commit et arrivé juste après. Au-delà, le secret est effacé (confidentialité
/// persistante).
const MAX_PAST_EPOCHS: usize = 3;

/// Ce que les groupes peuvent refuser.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GroupError {
    /// Ce groupe n'existe pas sur cet appareil (ou l'appareil en a été retiré).
    #[error("unknown group")]
    UnknownGroup,
    /// Le message n'est pas un message MLS attendu ici.
    #[error("malformed or unexpected MLS message")]
    Malformed,
    /// Le *Welcome* ou le groupe n'est pas dans la suite de Tessel (ADR-001).
    #[error("group uses another ciphersuite")]
    WrongCiphersuite,
    /// Un commit de cet appareil attend déjà la réponse du serveur.
    #[error("a commit is already pending")]
    PendingCommit,
    /// Aucun commit en attente.
    #[error("no pending commit")]
    NoPendingCommit,
    /// Un message de cet appareil-ci, renvoyé par le serveur.
    #[error("own message")]
    OwnMessage,
    /// L'opération ne change rien au groupe.
    #[error("nothing to commit")]
    NothingToCommit,
    /// OpenMLS a refusé l'opération ou le message.
    #[error("MLS: {0}")]
    Mls(String),
    /// Une feuille ne porte pas un certificat d'appareil valide.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

fn mls(error: impl fmt::Display) -> GroupError {
    GroupError::Mls(error.to_string())
}

/// L'identifiant d'un groupe : aléatoire, sans rien dire de son contenu.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GroupId(Vec<u8>);

impl GroupId {
    /// Reprend un identifiant connu.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// Les octets de l'identifiant.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn of(group: &MlsGroup) -> Self {
        Self(group.group_id().as_slice().to_vec())
    }
}

impl fmt::Debug for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupId(")?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))?;
        write!(f, ")")
    }
}

/// Qui retirer d'un groupe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// Tous les appareils d'un compte.
    Account(AccountId),
    /// Un appareil d'un compte.
    Device(AccountId, DeviceId),
}

impl Removal {
    fn matches(&self, certificate: &DeviceCertificate) -> bool {
        match self {
            Self::Account(account) => certificate.account() == account,
            Self::Device(account, device) => {
                certificate.account() == account && certificate.device_id() == *device
            }
        }
    }
}

/// Une intention de commit, gardée jusqu'à ce que le serveur l'accepte, pour la
/// rejouer si un autre commit l'a devancée.
#[derive(Debug, Clone)]
enum Operation {
    Add(Vec<VerifiedKeyPackage>),
    Remove(Vec<Removal>),
    CommitProposals,
}

/// Un commit à envoyer au serveur, et le *Welcome* des appareils ajoutés.
#[derive(Debug, PartialEq, Eq)]
pub struct Outgoing {
    /// Le commit (`MLSMessage`), pour les membres actuels.
    pub commit: Vec<u8>,
    /// Le *Welcome* (`MLSMessage`), pour les appareils ajoutés.
    pub welcome: Option<Vec<u8>>,
}

/// Ce qu'un message reçu a produit.
#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
    /// Des données chiffrées par un membre (le format est celui de la story 02-05).
    Application {
        /// Le groupe.
        group: GroupId,
        /// L'appareil qui l'a envoyé.
        sender: DeviceCertificate,
        /// Les octets déchiffrés.
        data: Vec<u8>,
    },
    /// Une proposition (un membre qui part), gardée jusqu'au prochain commit.
    Proposal {
        /// Le groupe.
        group: GroupId,
        /// L'appareil qui l'a envoyée.
        sender: DeviceCertificate,
    },
    /// Un commit appliqué : le groupe passe à l'epoch suivante.
    Commit {
        /// Le groupe.
        group: GroupId,
        /// L'appareil qui l'a fait.
        sender: DeviceCertificate,
        /// Les appareils ajoutés.
        added: Vec<DeviceCertificate>,
        /// Les appareils retirés.
        removed: Vec<DeviceCertificate>,
        /// Cet appareil-ci a été retiré : le groupe est effacé de l'appareil.
        removed_self: bool,
    },
}

/// L'état d'un groupe sur l'appareil.
#[derive(Debug)]
pub(crate) struct Group {
    mls: MlsGroup,
    pending: Option<Operation>,
}

fn create_config() -> MlsGroupCreateConfig {
    MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        .capabilities(capabilities())
        .use_ratchet_tree_extension(true)
        .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(MAX_PAST_EPOCHS)
        .build()
}

fn join_config() -> MlsGroupJoinConfig {
    MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(MAX_PAST_EPOCHS)
        .build()
}

fn certificate_of(member: &Member) -> Result<DeviceCertificate, GroupError> {
    Ok(DeviceCertificate::from_credential(
        &member.credential,
        member.signature_key.as_slice(),
    )?)
}

fn certificate_at(group: &MlsGroup, index: LeafNodeIndex) -> Result<DeviceCertificate, GroupError> {
    certificate_of(&group.member_at(index).ok_or(GroupError::Malformed)?)
}

fn sender_certificate(group: &MlsGroup, sender: &Sender) -> Result<DeviceCertificate, GroupError> {
    match sender {
        Sender::Member(index) => certificate_at(group, *index),
        _ => Err(GroupError::Malformed),
    }
}

/// Vérifie chaque feuille qu'un commit fait entrer ou change, et rend les appareils
/// ajoutés et retirés (lus dans l'état d'avant le commit).
fn summarize(
    group: &MlsGroup,
    commit: &StagedCommit,
) -> Result<(Vec<DeviceCertificate>, Vec<DeviceCertificate>), GroupError> {
    let mut added = Vec::new();
    for add in commit.add_proposals() {
        let key_package = add.add_proposal().key_package();
        if key_package.ciphersuite() != CIPHERSUITE {
            return Err(GroupError::WrongCiphersuite);
        }
        let leaf = key_package.leaf_node();
        added.push(DeviceCertificate::from_credential(
            leaf.credential(),
            leaf.signature_key().as_slice(),
        )?);
    }
    let updated = commit
        .update_proposals()
        .map(|update| update.update_proposal().leaf_node().clone())
        .chain(commit.update_path_leaf_node().cloned());
    for leaf in updated {
        DeviceCertificate::from_credential(leaf.credential(), leaf.signature_key().as_slice())?;
    }
    let removed = commit
        .remove_proposals()
        .map(|r| certificate_at(group, r.remove_proposal().removed()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((added, removed))
}

fn serialize(message: MlsMessageOut) -> Result<Vec<u8>, GroupError> {
    message.tls_serialize_detached().map_err(mls)
}

impl Group {
    /// Prépare le commit d'une intention sur l'état actuel du groupe. `None` quand
    /// l'intention n'a plus d'objet (les appareils sont déjà là, ou déjà partis).
    fn stage(
        &mut self,
        provider: &OpenMlsRustCrypto,
        device: &Device,
        operation: Operation,
    ) -> Result<Option<Outgoing>, GroupError> {
        if self.mls.pending_commit().is_some() {
            return Err(GroupError::PendingCommit);
        }
        let members: Vec<(LeafNodeIndex, DeviceCertificate)> = self
            .mls
            .members()
            .map(|m| Ok((m.index, certificate_of(&m)?)))
            .collect::<Result<_, GroupError>>()?;
        let own = self.mls.own_leaf_index();

        let (commit, welcome) = match &operation {
            Operation::Add(key_packages) => {
                let fresh: Vec<KeyPackage> = key_packages
                    .iter()
                    .filter(|kp| {
                        let wanted = kp.certificate();
                        !members.iter().any(|(_, c)| {
                            c.account() == wanted.account() && c.device_id() == wanted.device_id()
                        })
                    })
                    .map(|kp| kp.key_package().clone())
                    .collect();
                if fresh.is_empty() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .add_members(provider, device, &fresh)
                    .map_err(mls)?;
                (commit, Some(welcome))
            }
            Operation::Remove(removals) => {
                let leaves: Vec<LeafNodeIndex> = members
                    .iter()
                    .filter(|(index, c)| *index != own && removals.iter().any(|r| r.matches(c)))
                    .map(|(index, _)| *index)
                    .collect();
                if leaves.is_empty() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .remove_members(provider, device, &leaves)
                    .map_err(mls)?;
                (commit, welcome)
            }
            Operation::CommitProposals => {
                if self.mls.pending_proposals().next().is_none() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .commit_to_pending_proposals(provider, device)
                    .map_err(mls)?;
                (commit, welcome)
            }
        };
        self.pending = Some(operation);
        Ok(Some(Outgoing {
            commit: serialize(commit)?,
            welcome: welcome.map(serialize).transpose()?,
        }))
    }
}

impl Client {
    fn commit(&mut self, group: &GroupId, operation: Operation) -> Result<Outgoing, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        state
            .stage(provider, device, operation)?
            .ok_or(GroupError::NothingToCommit)
    }

    /// Crée un groupe dont cet appareil est le seul membre.
    pub fn create_group(&mut self) -> Result<GroupId, GroupError> {
        let mls = MlsGroup::new(
            &self.provider,
            &self.device,
            &create_config(),
            self.device.credential_with_key()?,
        )
        .map_err(mls)?;
        let id = GroupId::of(&mls);
        self.groups.insert(id.clone(), Group { mls, pending: None });
        Ok(id)
    }

    /// Prépare l'ajout d'appareils (ceux d'un nouveau membre, ou un nouvel appareil d'un
    /// membre), par leurs *KeyPackages* vérifiés. Les appareils déjà présents sont ignorés.
    pub fn add_devices(
        &mut self,
        group: &GroupId,
        key_packages: &[VerifiedKeyPackage],
    ) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::Add(key_packages.to_vec()))
    }

    /// Prépare le retrait d'appareils ou de comptes entiers. Cet appareil-ci n'est
    /// jamais retiré ainsi : il part avec [`Client::leave_group`].
    pub fn remove_devices(
        &mut self,
        group: &GroupId,
        removals: &[Removal],
    ) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::Remove(removals.to_vec()))
    }

    /// Prépare le commit des propositions reçues (un membre qui part).
    pub fn commit_pending_proposals(&mut self, group: &GroupId) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::CommitProposals)
    }

    /// Le serveur a accepté le commit : le groupe passe à l'epoch suivante.
    pub fn commit_accepted(&mut self, group: &GroupId) -> Result<(), GroupError> {
        let provider = &self.provider;
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        if state.mls.pending_commit().is_none() {
            return Err(GroupError::NoPendingCommit);
        }
        state.mls.merge_pending_commit(provider).map_err(mls)?;
        state.pending = None;
        Ok(())
    }

    /// Le serveur a refusé le commit (un autre l'a devancé dans cette epoch). Le
    /// commit est abandonné, l'intention gardée : traiter les commits qui l'ont
    /// devancé, puis [`Client::retry_commit`].
    pub fn commit_rejected(&mut self, group: &GroupId) -> Result<(), GroupError> {
        let provider = &self.provider;
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        if state.mls.pending_commit().is_none() {
            return Err(GroupError::NoPendingCommit);
        }
        state
            .mls
            .clear_pending_commit(provider.storage())
            .map_err(mls)
    }

    /// Rejoue l'intention d'un commit refusé sur l'état à jour. `None` quand elle n'a
    /// plus d'objet (les commits reçus entre-temps l'ont déjà accomplie).
    pub fn retry_commit(&mut self, group: &GroupId) -> Result<Option<Outgoing>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        let Some(operation) = state.pending.take() else {
            return Ok(None);
        };
        state.stage(provider, device, operation)
    }

    /// Quitter un groupe : rend une proposition à envoyer aux membres. Un autre membre
    /// la fait entrer par un commit ; cet appareil est retiré en le traitant.
    pub fn leave_group(&mut self, group: &GroupId) -> Result<Vec<u8>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        serialize(state.mls.leave_group(provider, device).map_err(mls)?)
    }

    /// Rejoint un groupe par le *Welcome* reçu. Refuse une autre suite que celle de
    /// Tessel, et un groupe dont un membre ne porte pas de certificat valide.
    pub fn join_group(&mut self, welcome: &[u8]) -> Result<GroupId, GroupError> {
        let message =
            MlsMessageIn::tls_deserialize_exact(welcome).map_err(|_| GroupError::Malformed)?;
        let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
            return Err(GroupError::Malformed);
        };
        // Avant de toucher aux secrets : une autre suite n'entre pas (ADR-001).
        if welcome.ciphersuite() != CIPHERSUITE {
            return Err(GroupError::WrongCiphersuite);
        }
        let staged = StagedWelcome::new_from_welcome(&self.provider, &join_config(), welcome, None)
            .map_err(mls)?;
        for member in staged.members() {
            certificate_of(&member)?;
        }
        let mls = staged.into_group(&self.provider).map_err(mls)?;
        let id = GroupId::of(&mls);
        self.groups.insert(id.clone(), Group { mls, pending: None });
        Ok(id)
    }

    /// Chiffre des données pour le groupe (le format est celui de la story 02-05).
    pub fn encrypt(&mut self, group: &GroupId, data: &[u8]) -> Result<Vec<u8>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        serialize(
            state
                .mls
                .create_message(provider, device, data)
                .map_err(mls)?,
        )
    }

    /// Traite un message reçu d'un groupe : données, proposition ou commit.
    pub fn process(&mut self, message: &[u8]) -> Result<Incoming, GroupError> {
        let message =
            MlsMessageIn::tls_deserialize_exact(message).map_err(|_| GroupError::Malformed)?;
        let message = message
            .try_into_protocol_message()
            .map_err(|_| GroupError::Malformed)?;
        let id = GroupId(message.group_id().as_slice().to_vec());
        let provider = &self.provider;
        let state = self.groups.get_mut(&id).ok_or(GroupError::UnknownGroup)?;
        let processed = state.mls.process_message(provider, message).map_err(mls)?;
        let sender = sender_certificate(&state.mls, processed.sender())?;

        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(data) => Ok(Incoming::Application {
                group: id,
                sender,
                data: data.into_bytes(),
            }),
            ProcessedMessageContent::ProposalMessage(proposal) => {
                state
                    .mls
                    .store_pending_proposal(provider.storage(), *proposal)
                    .map_err(mls)?;
                Ok(Incoming::Proposal { group: id, sender })
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => Err(GroupError::Malformed),
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                let (added, removed) = summarize(&state.mls, &commit)?;
                let removed_self = commit.self_removed();
                // Un commit d'un autre a gagné cette epoch : le nôtre, s'il y en avait
                // un, est caduc ; son intention reste pour `retry_commit`.
                if state.mls.pending_commit().is_some() {
                    state
                        .mls
                        .clear_pending_commit(provider.storage())
                        .map_err(mls)?;
                }
                state
                    .mls
                    .merge_staged_commit(provider, *commit)
                    .map_err(mls)?;
                if removed_self {
                    self.forget(&id)?;
                }
                Ok(Incoming::Commit {
                    group: id,
                    sender,
                    added,
                    removed,
                    removed_self,
                })
            }
            // Notre propre commit renvoyé par le serveur : il l'a accepté. (Seulement en
            // `PublicMessage` ; chiffré, son auteur ne peut pas le relire et l'acceptation
            // passe par `commit_accepted`.)
            ProcessedMessageContent::OwnPendingCommit => {
                let commit = state
                    .mls
                    .pending_commit()
                    .ok_or(GroupError::NoPendingCommit)?;
                let (added, removed) = summarize(&state.mls, commit)?;
                state.mls.merge_pending_commit(provider).map_err(mls)?;
                state.pending = None;
                Ok(Incoming::Commit {
                    group: id,
                    sender,
                    added,
                    removed,
                    removed_self: false,
                })
            }
            ProcessedMessageContent::OwnPrivateMessage => Err(GroupError::OwnMessage),
        }
    }

    /// Les appareils membres du groupe.
    pub fn members(&self, group: &GroupId) -> Result<Vec<DeviceCertificate>, GroupError> {
        let state = self.groups.get(group).ok_or(GroupError::UnknownGroup)?;
        state.mls.members().map(|m| certificate_of(&m)).collect()
    }

    /// L'epoch actuelle du groupe.
    pub fn epoch(&self, group: &GroupId) -> Result<u64, GroupError> {
        let state = self.groups.get(group).ok_or(GroupError::UnknownGroup)?;
        Ok(state.mls.epoch().as_u64())
    }

    /// Efface un groupe de l'appareil, état MLS compris.
    fn forget(&mut self, group: &GroupId) -> Result<(), GroupError> {
        if let Some(mut state) = self.groups.remove(group) {
            state.mls.delete(self.provider.storage()).map_err(mls)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
