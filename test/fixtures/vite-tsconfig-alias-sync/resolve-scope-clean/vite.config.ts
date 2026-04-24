export default {
  test: {
    alias: {
      "@app": "./src/test-only"
    }
  },
  resolve: {
    alias: {
      "@app": "./src/shared"
    }
  }
};
