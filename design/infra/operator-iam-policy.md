# Operator IAM Policy

This template uses API Gateway IAM authorization for account lifecycle routes. Operators should call these routes with SigV4-signed requests.

Use the deployed `AdminRouteArnPattern` output as the resource base for a minimal operator policy.

Example policy:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "InvokeIrongateAdminLifecycleRoutes",
      "Effect": "Allow",
      "Action": "execute-api:Invoke",
      "Resource": [
        "arn:aws:execute-api:REGION:ACCOUNT_ID:API_ID/STAGE/GET/_admin/users/*",
        "arn:aws:execute-api:REGION:ACCOUNT_ID:API_ID/STAGE/POST/_admin/users/*/disable",
        "arn:aws:execute-api:REGION:ACCOUNT_ID:API_ID/STAGE/POST/_admin/users/*/delete",
        "arn:aws:execute-api:REGION:ACCOUNT_ID:API_ID/STAGE/POST/_admin/users/*/revoke-sessions"
      ]
    }
  ]
}
```

Keep this policy separate from deploy permissions. It is for operator lifecycle calls only and should not grant broader API invocation or raw DynamoDB table access.
