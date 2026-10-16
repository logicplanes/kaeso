export async function sendTraffic(jobs, refused, workers = 6) {
  const queue = [...jobs.map((run) => ({ run, refused: false })), ...refused.map((run) => ({ run, refused: true }))];
  const total = queue.length;
  const wrong = [];
  await Promise.all(
    Array.from({ length: workers }, async () => {
      for (let job = queue.shift(); job; job = queue.shift()) {
        const outcome = await job.run().then(
          () => ({ ok: true }),
          (error) => ({ ok: false, error }),
        );
        if (!outcome.ok && !job.refused) wrong.push(outcome.error instanceof Error ? outcome.error.message : String(outcome.error));
        if (outcome.ok && job.refused) wrong.push("a request meant to be refused went through");
      }
    }),
  );
  if (wrong.length) throw new Error(`seed: ${wrong.length} of ${total} requests didn't go as planned; the first: ${wrong[0]}`);
  return { sent: total, refused: refused.length };
}
