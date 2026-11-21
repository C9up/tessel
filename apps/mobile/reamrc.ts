// A plain object rather than `defineConfig`: @c9up/ream 0.2.31 does not type the
// `mobile` block that ream-cli 0.3.22 reads yet (reported to Ream).
export default {
  mobile: {
    name: 'Tessel',
    id: 'com.c9up.tessel',
    entry: 'mobile/index.ts',
    version: '0.1.0',
    build: 1,
    android: {
      abis: ['arm64-v8a', 'x86_64'],
    },
  },
}
