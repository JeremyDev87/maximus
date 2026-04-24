export default {
  resolve: {
    alias: [
      ({ find: "@generated", replacement: "./generated" }),
      { find: /^~(.+)/, replacement: "./legacy/$1" },
      { find: "@app", replacement: "./src/client" }
    ]
  }
};
