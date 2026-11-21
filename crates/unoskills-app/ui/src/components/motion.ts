import { reduceMotion } from "nsh";

export const controlMotionClass = "motion-safe:transition-colors motion-safe:duration-120 motion-safe:ease-out";

export function dialogAnimation() {
  return reduceMotion() ? "none" : "fade";
}
