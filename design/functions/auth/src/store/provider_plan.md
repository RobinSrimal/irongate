# Private Provider Plan

Source: [packages/functions/auth/src/store/provider_plan.rs](../../../../../packages/functions/auth/src/store/provider_plan.rs).

## What

Internal new-identity snapshots and conditional operations shared by direct Google/Apple issuance
and durable Google email confirmation.

## Why

Waiting for proof must not select a newer owner, subject or credential generation. One typed plan
keeps direct and deferred issuance on the same ownership rules without exposing raw transactions.

## How

`NewProviderPlan` captures provider/identity digest, absent or exact deleted identity, the complete
candidate/existing active account, email ownership plan and creation time. Store-only constructors
read identity first, reject active identities and enforce deleted-identity and email-owner reuse
independently. Missing/inactive/corrupt owner bindings fail closed through the ownership module.

The five operations guard/create the account, create the identity subject index, guard/create email
owner and reverse index, then write the identity with an absence/exact-tombstone condition.
Direct issuance appends the code; confirmation appends code and exact pending-record deletion.
`creation_guards` derives checks from the same five conditions without making those writes.

The private serialized snapshot is data, not caller-selected keys or transaction operations.
Google confirmation validates provider/digest/time binding, active account shape, fresh-account
generation zero, deleted-identity shape and ownership bindings on load. Account generation is
required. Finishing uses captured values without another resolver call or retry against newer state.
`PreparedProviderIssuance` itself is not serializable authority.
