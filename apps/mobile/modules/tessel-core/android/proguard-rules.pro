# Règles appliquées à l'app qui utilise ce module (consumerProguardFiles).

# JNA charge ses classes par réflexion et référence java.awt, absent d'Android.
-dontwarn java.awt.**
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }

# Les liaisons uniffi : leurs structures et rappels sont lus par JNA par réflexion.
-keep class uniffi.** { *; }
