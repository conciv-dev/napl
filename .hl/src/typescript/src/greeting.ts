export function greet(name: string): string {
  const trimmed = name.trim();
  if (trimmed.length === 0) {
    throw new Error("Name must not be empty");
  }
  return `Hello, ${trimmed}!`;
}
