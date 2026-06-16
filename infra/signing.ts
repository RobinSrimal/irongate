import { authSigningKmsPermissions, infraConfig } from "./config.js";

const signingKeyAliasName = `alias/${$app.name}/auth-signing-${$app.stage}`;

const signingKmsKey =
  infraConfig.signingMode === "kms-es256"
    ? new aws.kms.Key("AuthSigningKmsKey", {
        description: `${$app.name} ${$app.stage} auth token signing key`,
        deletionWindowInDays: 30,
        customerMasterKeySpec: "ECC_NIST_P256",
        keyUsage: "SIGN_VERIFY",
      })
    : undefined;

const signingKmsAlias = signingKmsKey
  ? new aws.kms.Alias("AuthSigningKmsAlias", {
      name: signingKeyAliasName,
      targetKeyId: signingKmsKey.keyId,
    })
  : undefined;

export const signingEnvironment =
  infraConfig.signingMode === "kms-es256" && signingKmsKey && signingKmsAlias
    ? {
        AUTH_SIGNING_MODE: "kms-es256",
        AUTH_SIGNING_KEY_ID: infraConfig.signingKeyId,
        AUTH_SIGNING_KMS_KEY_ID: signingKeyAliasName,
      }
    : (() => {
        const signingPrivateKey = new sst.Secret("AuthSigningPrivateKey");
        return {
          AUTH_SIGNING_MODE: "local-es256",
          AUTH_SIGNING_KEY_ID: infraConfig.signingKeyId,
          AUTH_SIGNING_PRIVATE_KEY_SECRET: "AUTH_SIGNING_PRIVATE_KEY",
          AUTH_SIGNING_PRIVATE_KEY: signingPrivateKey.value,
        };
      })();

export const signingKmsPermissions = signingKmsKey
  ? [authSigningKmsPermissions(signingKmsKey.arn)]
  : [];

export const signingKmsKeyArn = signingKmsKey?.arn ?? "local-es256";
