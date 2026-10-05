const [entityId, writerText] = Bun.argv.slice(2);
const writer = Number(writerText);
const start = Number(Bun.env.JADPO_ENTITY_DOSSIER_WRITER_START);
if (!entityId || !Number.isInteger(writer) || writer < 0 || writer > 1 || !Number.isSafeInteger(start)) {
  throw new Error("Invalid entity-dossier writer arguments");
}

const { persistence } = await import("../../compile/pass/build/target/persistence.ts");
await Promise.all(Array.from({ length: 12 }, (_, sequence) => {
  const index = writer * 12 + sequence;
  const email = `process-${writer}-${sequence}@example.com`;
  const operationTime = new Date(start + (index + 1) * 1000).toISOString();
  return persistence.withOperationTime(operationTime).transaction(async (transaction: typeof persistence) => {
    await transaction.update_required_Customer_by_id_set_email(entityId, email);
  });
}));
