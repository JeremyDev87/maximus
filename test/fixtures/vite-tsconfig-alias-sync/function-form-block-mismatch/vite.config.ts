function defineConfig(value: unknown) {
  return value;
}

export default defineConfig(() => {
  return {
    resolve: {
      alias: {
        "@app": "./src/client",
      },
    },
  };
});
