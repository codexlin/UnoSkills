import type { StackMember } from "./host";

export function stackMemberRows<T>(items: readonly T[], memberFor: (item: T) => StackMember) {
  const occurrences = new Map<string, number>();
  return items.map((item, originalIndex) => {
    const { name, source, note } = memberFor(item);
    const identity = JSON.stringify([name, source, note]);
    // TOML can repeat identical members. An occurrence keeps both rows addressable.
    const occurrence = occurrences.get(identity) ?? 0;
    occurrences.set(identity, occurrence + 1);
    return { key: `${identity}:${occurrence}`, item, originalIndex };
  });
}
