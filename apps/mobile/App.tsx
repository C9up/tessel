import { StatusBar } from 'expo-status-bar'
import { StyleSheet, Text, View } from 'react-native'
import TesselCore from './modules/tessel-core/src/TesselCoreModule'

/** Écran provisoire : il prouve que le cœur Rust répond depuis l'app (story 01-04). */
export default function App() {
  return (
    <View style={styles.container}>
      <Text>Tessel</Text>
      <Text testID="core-version">tessel-core {TesselCore.version()}</Text>
      <StatusBar style="auto" />
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#fff',
    alignItems: 'center',
    justifyContent: 'center',
  },
})
