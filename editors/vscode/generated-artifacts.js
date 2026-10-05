"use strict";

const GENERATED_ARTIFACTS = Object.freeze([
  { label: "Application metadata", path: "app.meta.json" },
  { label: "Route inventory", path: "inventory/routes.json" },
  { label: "Callable inventory", path: "inventory/callables.json" },
  { label: "Failure audit", path: "audit/failures.json" },
  { label: "Entity audit", path: "audit/entities.json" },
  { label: "Transaction audit", path: "audit/transactions.json" },
  { label: "Configuration audit", path: "audit/configuration.json" },
  { label: "Policy audit", path: "audit/policy.json" },
  { label: "Authentication audit", path: "audit/authentication.json" },
  { label: "Validation plan", path: "validators/plan.json" },
  { label: "Public failure compatibility", path: "compatibility/public-failure-codes.json" },
  { label: "OpenAPI document", path: "openapi/openapi.json" },
  { label: "Diagnostic catalogue", path: "diagnostics/catalogue.json" },
  { label: "Diagnostic reference", path: "diagnostics/reference.md" },
].map(item => Object.freeze(item)));

module.exports = { GENERATED_ARTIFACTS };
