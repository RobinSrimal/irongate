export const authSecrets = {
  hmacLookupSecret: new sst.Secret("AuthHmacLookupSecret"),
  resendApiKey: new sst.Secret("ResendApiKey"),
  googleClientSecret: new sst.Secret("GoogleClientSecret"),
  applePrivateKey: new sst.Secret("ApplePrivateKey"),
};
