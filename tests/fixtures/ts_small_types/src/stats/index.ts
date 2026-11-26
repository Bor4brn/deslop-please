export { Rate } from "./rate";
export { Tally } from "./tally";
export { Window } from "./window";

export function percent(part: number, whole: number): number {
  return whole ? Math.floor((part * 100) / whole) : 0;
}
