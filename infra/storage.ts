export const table = new sst.aws.Dynamo("AuthTable", {
  fields: {
    pk: "string",
    sk: "string",
  },
  primaryIndex: { hashKey: "pk", rangeKey: "sk" },
  ttl: "expiry",
});
