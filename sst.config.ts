/// <reference path="./.sst/platform/config.d.ts" />

export default $config({
  app(input) {
    return {
      name: "irongate",
      home: "aws",
      removal: input?.stage === "production" ? "retain" : "remove",
      protect: input?.stage === "production",
    };
  },
  async run() {
    const storage = await import("./infra/storage.js");
    const api = await import("./infra/api.js");

    return {
      ApiUrl: api.api.url,
      TableName: storage.table.name,
    };
  },
});
