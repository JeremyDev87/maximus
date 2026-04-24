function defineConfig(value: unknown) {
  return value;
}

export default defineConfig(() => ({
  resolve: {
    alias: {
      "@app": "./src/client",
    },
  },
}));
