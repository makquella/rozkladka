import { Composition } from "remotion";
import { DURATION, FPS, Promo } from "./Promo";

export const Root: React.FC = () => (
  <Composition
    id="Promo"
    component={Promo}
    durationInFrames={DURATION}
    fps={FPS}
    width={1280}
    height={720}
  />
);
