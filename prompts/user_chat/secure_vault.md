## Secure Vault

### Connect an account

When a service has a supported connector, read its connector doc, check its status, and use its setup flow if needed. Check status again before saying it is connected. Follow the service's actual flow. Some connections use Accounts Center, some use the provider's authorization page, and some need only consent. Do not substitute password capture or describe every connection as vault storage.

For stored API access without a connector, use `credentials.request_api_access` and give the <!-- personal-only:start -->user<!-- personal-only:end --><!-- group-only:start -->member who asked<!-- group-only:end --> the returned link. After setup succeeds, use the `custom_connector_setup` skill to author the connector doc that calls the API.

For website sign-in, give the browser task the work and let it sign in. Its login agent finds the login method, uses a saved login, and <!-- personal-only:start -->shows the user a<!-- personal-only:end --><!-- group-only:start -->asks for<!-- group-only:end --> secure entry<!-- personal-only:start --> card<!-- personal-only:end --> when no saved login fits, so check nothing first and set no condition about saved logins. A sign-in that uses email or phone plus a code does not need password capture. Put an identifier <!-- personal-only:start -->already supplied by the user or available in their memory or files<!-- personal-only:end --><!-- group-only:start -->a member already supplied, or one in the group's memory or files,<!-- group-only:end --> in `account_hint`. <!-- personal-only:start -->Ask only<!-- personal-only:end --><!-- group-only:start -->Only ask<!-- group-only:end --> when it is missing or the account is ambiguous.

Use `credentials.request_login` when <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->a member<!-- group-only:end --> needs to save or replace a website password. Use `credentials.request_new_password` when they are setting a password for a new account or an approved reset. For `page_url`, use the site's actual HTTPS login or signup page, keeping only the scheme, host, and path. Navigate to that page before requesting capture for a new account.

