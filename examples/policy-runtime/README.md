# Policy runtime example

This project demonstrates one protected entity serving audiences with different
field visibility without returning a role-dependent shape from one operation.

`User.summary` admits any authenticated principal and returns the stable
`UserSummary` projection. `User.administrative_details` is narrowed to
`ApplicationRole.administrator` and returns `UserAdministrativeDetails`, whose
required `private_email` field is readable only by that role.

The intended HTTP bindings are two independently typed routes:

```text
GET /users/{id}        -> UserSummary
GET /admin/users/{id}  -> UserAdministrativeDetails
```

Protected route generation remains fail-closed until a real authentication
adapter is configured. The generated persistence runtime test therefore calls
the two policy-scoped operations directly. It proves that a regular principal
can obtain the summary row, cannot obtain the administrative row, and that an
administrator can obtain the complete required administrative projection.

The compiler fixture
`tests/compile/fail/152_policy_output_field_unproved.jadpo` supplies the negative
case: an operation admitting a broader role while returning an output that
contains its restricted required field fails with
`POLICY_OUTPUT_FIELD_UNPROVED`.
