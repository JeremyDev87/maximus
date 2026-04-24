const path = {
  resolve: (...segments: string[]) => segments.join("/"),
};
const __dirname = ".";

export default {
  resolve: {
    alias: {
      "@app": path.resolve(__dirname, "./src/client"),
    },
  },
};
