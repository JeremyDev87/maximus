export async function applyFixes(fixes) {
  const applied = [];

  for (const fix of fixes) {
    const result = await fix.apply();
    applied.push({
      id: fix.id,
      title: fix.title,
      files: fix.files,
      ...result,
    });
  }

  return applied;
}
