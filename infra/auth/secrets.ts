export const authSecrets = {
  hmacLookupSecret: new sst.Secret("AuthHmacLookupSecret"),
  resendApiKey: new sst.Secret("ResendApiKey"),
};
