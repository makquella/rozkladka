import { loadFont } from "@remotion/google-fonts/Inter";
import React from "react";
import {
  AbsoluteFill,
  Easing,
  Sequence,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";

const { fontFamily } = loadFont("normal", {
  weights: ["400", "500", "600", "700"],
  subsets: ["latin", "cyrillic"],
});

export const FPS = 30;

// Scene boundaries, in frames.
const S1 = { from: 0, length: 150 };
const S2 = { from: 150, length: 112 };
const S3 = { from: 262, length: 128 };
const S4 = { from: 390, length: 96 };
export const DURATION = S4.from + S4.length;

// A quiet palette: warm paper, ink, one soft green.
const PAPER = "#f7f7f5";
const INK = "#111111";
const MUTED = "#8a8a8a";
const LINE = "#e4e4e0";
const GREEN = "#10a37f";
const GREEN_SOFT = "#e6f5ef";
const RED = "#e5484d";

const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
const ease = Easing.bezier(0.25, 0.1, 0.25, 1);

/* ------------------------------------------------------------------ building blocks */

/** Scene wrapper: a short fade in and out. */
const Scene: React.FC<{ length: number; children: React.ReactNode }> = ({ length, children }) => {
  const frame = useCurrentFrame();
  const opacity =
    interpolate(frame, [0, 10], [0, 1], clamp) * interpolate(frame, [length - 10, length], [1, 0], clamp);
  return <AbsoluteFill style={{ opacity, fontFamily }}>{children}</AbsoluteFill>;
};

const Headline: React.FC<{ text: string; at?: number; top?: number; size?: number; color?: string; weight?: number }> = ({
  text,
  at = 0,
  top = 170,
  size = 44,
  color = INK,
  weight = 600,
}) => {
  const frame = useCurrentFrame();
  const p = interpolate(frame, [at, at + 14], [0, 1], { ...clamp, easing: ease });
  return (
    <div
      style={{
        position: "absolute",
        top,
        width: "100%",
        textAlign: "center",
        fontSize: size,
        fontWeight: weight,
        letterSpacing: -0.8,
        color,
        opacity: p,
        transform: `translateY(${(1 - p) * 8}px)`,
      }}
    >
      {text}
    </div>
  );
};

/** One character that softly swaps from `from` to `to` around `at`. */
const SwapChar: React.FC<{ from: string; to: string; at: number }> = ({ from, to, at }) => {
  const frame = useCurrentFrame();
  const out = interpolate(frame, [at, at + 6], [0, 1], { ...clamp, easing: ease });
  const inn = interpolate(frame, [at + 4, at + 11], [0, 1], { ...clamp, easing: ease });
  const tint = interpolate(frame, [at + 4, at + 30], [1, 0], clamp);
  const cell = (c: string) => (c === " " ? " " : c);
  return (
    <span style={{ position: "relative", display: "inline-block" }}>
      {/* The new character sets the width once it is in. */}
      <span
        style={{
          display: "inline-block",
          opacity: inn,
          transform: `translateY(${(1 - inn) * 10}px)`,
          color: inn > 0 ? mix(INK, GREEN, tint) : INK,
          visibility: inn > 0 ? "visible" : "hidden",
        }}
      >
        {cell(inn > 0 ? to : from)}
      </span>
      {out < 1 && (
        <span
          style={{
            position: "absolute",
            left: 0,
            top: 0,
            opacity: 1 - out,
            transform: `translateY(${-out * 10}px)`,
          }}
        >
          {cell(from)}
        </span>
      )}
    </span>
  );
};

/** Text typed one key at a time, then swapped to its other-layout version. */
const TypedText: React.FC<{
  from: string;
  to: string;
  typeAt: number;
  perChar: number;
  morphAt: number;
  selectAt?: number;
  /** Red spell-check underline under the wrong-layout text until it is fixed. */
  squiggle?: boolean;
}> = ({ from, to, typeAt, perChar, morphAt, selectAt, squiggle }) => {
  const frame = useCurrentFrame();
  const typed = Math.max(0, Math.min(from.length, Math.floor((frame - typeAt) / perChar) + 1));
  const typing = frame >= typeAt && typed < from.length;
  const caretOn = typing || Math.floor(frame / 16) % 2 === 0;
  const doneAt = morphAt + from.length * 2 + 12;
  const typedAt = typeAt + (from.length - 1) * perChar;
  const wavy = squiggle
    ? interpolate(frame, [typedAt + 4, typedAt + 12], [0, 1], clamp) *
      interpolate(frame, [morphAt, morphAt + 6], [1, 0], clamp)
    : 0;
  const selection =
    selectAt === undefined
      ? 0
      : interpolate(frame, [selectAt, selectAt + 12], [0, 1], { ...clamp, easing: ease }) *
        interpolate(frame, [doneAt, doneAt + 8], [1, 0], clamp);
  return (
    <div style={{ position: "relative", display: "flex", alignItems: "center", height: 72 }}>
      {selection > 0 && (
        <div
          style={{
            position: "absolute",
            left: -4,
            top: 8,
            bottom: 8,
            width: `calc(${selection * 100}% - 4px)`,
            background: "#d6e6ff",
            borderRadius: 4,
          }}
        />
      )}
      <div style={{ position: "relative", whiteSpace: "pre" }}>
        {wavy > 0 && (
          <svg
            style={{ position: "absolute", left: 0, bottom: -4, width: "100%", height: 8, opacity: wavy }}
            preserveAspectRatio="none"
            viewBox="0 0 120 8"
          >
            <path
              d="M0 4 Q5 0 10 4 T20 4 T30 4 T40 4 T50 4 T60 4 T70 4 T80 4 T90 4 T100 4 T110 4 T120 4"
              stroke={RED}
              strokeWidth="1.6"
              fill="none"
            />
          </svg>
        )}
        {from
          .slice(0, typed)
          .split("")
          .map((c, i) => (
            <SwapChar key={i} from={c} to={to[i] ?? c} at={morphAt + i * 2} />
          ))}
      </div>
      {frame >= typeAt - 10 && (
        <div style={{ width: 3, height: 52, marginLeft: 3, background: INK, opacity: caretOn ? 1 : 0 }} />
      )}
    </div>
  );
};

/** Small layout indicator that softly changes label and color. */
const LayoutBadge: React.FC<{ from: string; to: string; at: number }> = ({ from, to, at }) => {
  const frame = useCurrentFrame();
  const p = interpolate(frame, [at, at + 8], [0, 1], { ...clamp, easing: ease });
  const showNew = p >= 0.5;
  const fade = showNew ? (p - 0.5) * 2 : 1 - p * 2;
  return (
    <div
      style={{
        minWidth: 74,
        textAlign: "center",
        padding: "8px 14px",
        borderRadius: 10,
        fontSize: 18,
        fontWeight: 600,
        letterSpacing: 1,
        color: showNew ? GREEN : "#666",
        background: showNew ? GREEN_SOFT : "#f1f1ef",
      }}
    >
      <span style={{ opacity: fade }}>{showNew ? to : from}</span>
    </div>
  );
};

/** The input field the demo types into. */
const InputCard: React.FC<{ children: React.ReactNode; badge: React.ReactNode }> = ({ children, badge }) => (
  <div
    style={{
      position: "absolute",
      left: 240,
      top: 300,
      width: 800,
      height: 120,
      boxSizing: "border-box",
      borderRadius: 20,
      padding: "0 28px 0 36px",
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      background: "white",
      border: `1px solid ${LINE}`,
      boxShadow: "0 1px 2px rgba(0,0,0,0.04), 0 8px 28px rgba(0,0,0,0.06)",
      fontSize: 52,
      fontWeight: 500,
      letterSpacing: -0.5,
      color: INK,
    }}
  >
    {children}
    {badge}
  </div>
);

/** A plain keyboard key that is pressed at the given frames. */
const KeyCap: React.FC<{ label: string; presses: number[]; appearAt: number; width?: number }> = ({
  label,
  presses,
  appearAt,
  width = 220,
}) => {
  const frame = useCurrentFrame();
  const enter = interpolate(frame, [appearAt, appearAt + 10], [0, 1], { ...clamp, easing: ease });
  const down = presses.some((at) => frame >= at && frame < at + 6);
  return (
    <div
      style={{
        width,
        height: 60,
        boxSizing: "border-box",
        borderRadius: 12,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        fontSize: 22,
        fontWeight: 500,
        color: "#333",
        background: down ? "#f0f0ee" : "white",
        border: `1px solid ${LINE}`,
        borderBottom: `${down ? 1 : 4}px solid #d6d6d2`,
        transform: `translateY(${down ? 3 : (1 - enter) * 8}px)`,
        opacity: enter,
      }}
    >
      {label}
    </div>
  );
};

/** A quiet confirmation line under the field. */
const Check: React.FC<{ at: number; label: string }> = ({ at, label }) => {
  const frame = useCurrentFrame();
  const p = interpolate(frame, [at, at + 12], [0, 1], { ...clamp, easing: ease });
  return (
    <div
      style={{
        position: "absolute",
        left: 0,
        right: 0,
        top: 452,
        textAlign: "center",
        fontSize: 22,
        fontWeight: 500,
        color: GREEN,
        opacity: p,
        transform: `translateY(${(1 - p) * 6}px)`,
      }}
    >
      ✓ {label}
    </div>
  );
};

const KeyRow: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div style={{ position: "absolute", left: 0, right: 0, top: 470, display: "flex", justifyContent: "center", gap: 14 }}>
    {children}
  </div>
);

/** Fades its children out at `at` (the key row gives way to the confirmation). */
const FadeOut: React.FC<{ at: number; children: React.ReactNode }> = ({ at, children }) => {
  const frame = useCurrentFrame();
  return <div style={{ opacity: interpolate(frame, [at, at + 8], [1, 0], clamp) }}>{children}</div>;
};

/* ------------------------------------------------------------------ scenes */

const SceneOne: React.FC = () => (
  <Scene length={S1.length}>
    <Headline text="Набрав не в тій розкладці?" at={4} />
    <Headline text="Просто натисни пробіл." at={96} top={228} size={30} color={MUTED} weight={500} />
    <InputCard badge={<LayoutBadge from="ENG" to="УКР" at={76} />}>
      <TypedText from="ghbdsn" to="привіт" typeAt={14} perChar={6} morphAt={70} squiggle />
    </InputCard>
    <FadeOut at={92}>
      <KeyRow>
        <KeyCap label="пробіл" presses={[60]} appearAt={44} width={360} />
      </KeyRow>
    </FadeOut>
    <Check at={100} label="виправлено автоматично" />
  </Scene>
);

const SceneTwo: React.FC = () => (
  <Scene length={S2.length}>
    <Headline text="І навпаки" at={4} />
    <InputCard badge={<LayoutBadge from="УКР" to="ENG" at={58} />}>
      <TypedText from="руддщ" to="hello" typeAt={12} perChar={6} morphAt={52} squiggle />
    </InputCard>
    <FadeOut at={74}>
      <KeyRow>
        <KeyCap label="пробіл" presses={[44]} appearAt={30} width={360} />
      </KeyRow>
    </FadeOut>
    <Check at={80} label="hello" />
  </Scene>
);

const SceneThree: React.FC = () => (
  <Scene length={S3.length}>
    <Headline text="Виділи текст і двічі натисни Ctrl" at={4} />
    <InputCard badge={<LayoutBadge from="ENG" to="УКР" at={72} />}>
      <TypedText from="zr cghfdb&" to="як справи?" typeAt={8} perChar={2} morphAt={66} selectAt={34} />
    </InputCard>
    <FadeOut at={98}>
      <KeyRow>
        <KeyCap label="Ctrl" presses={[50]} appearAt={40} width={130} />
        <KeyCap label="Ctrl" presses={[58]} appearAt={44} width={130} />
      </KeyRow>
    </FadeOut>
    <Check at={102} label="виправлено виділене" />
  </Scene>
);

const AppIcon: React.FC<{ size: number }> = ({ size }) => (
  <svg width={size} height={size} viewBox="0 0 32 32">
    <rect x="0" y="0" width="32" height="32" rx="8" fill={INK} />
    <path d="M8 12 H22 M18.5 8.5 L22 12 L18.5 15.5" stroke="white" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M24 20 H10 M13.5 16.5 L10 20 L13.5 23.5" stroke="white" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const SceneFour: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const icon = spring({ frame, fps, config: { damping: 20, stiffness: 140 } });
  const show = (at: number) => interpolate(frame, [at, at + 14], [0, 1], { ...clamp, easing: ease });
  const fadeOut = interpolate(frame, [S4.length - 8, S4.length], [1, 0], clamp);
  const [title, tags, link] = [show(8), show(18), show(28)];
  return (
    <AbsoluteFill
      style={{ fontFamily, alignItems: "center", justifyContent: "center", flexDirection: "column", opacity: fadeOut }}
    >
      <div style={{ transform: `scale(${0.85 + 0.15 * icon})`, opacity: Math.min(1, icon) }}>
        <AppIcon size={84} />
      </div>
      <div
        style={{
          marginTop: 28,
          fontSize: 72,
          fontWeight: 700,
          letterSpacing: -2,
          color: INK,
          opacity: title,
          transform: `translateY(${(1 - title) * 8}px)`,
        }}
      >
        Rozkladka
      </div>
      <div style={{ marginTop: 14, fontSize: 24, color: MUTED, opacity: tags }}>
        Безкоштовно · 300 КБ · Без реклами · Відкритий код
      </div>
      <div
        style={{
          marginTop: 40,
          padding: "12px 22px",
          borderRadius: 12,
          border: `1px solid ${LINE}`,
          background: "white",
          fontSize: 22,
          fontWeight: 500,
          color: INK,
          opacity: link,
        }}
      >
        github.com/makquella/rozkladka
      </div>
    </AbsoluteFill>
  );
};

export const Promo: React.FC = () => (
  <AbsoluteFill style={{ background: PAPER }}>
    <Sequence from={S1.from} durationInFrames={S1.length}>
      <SceneOne />
    </Sequence>
    <Sequence from={S2.from} durationInFrames={S2.length}>
      <SceneTwo />
    </Sequence>
    <Sequence from={S3.from} durationInFrames={S3.length}>
      <SceneThree />
    </Sequence>
    <Sequence from={S4.from} durationInFrames={S4.length}>
      <SceneFour />
    </Sequence>
  </AbsoluteFill>
);

/** Linear mix of two #rrggbb colors. */
function mix(a: string, b: string, t: number): string {
  const parse = (c: string) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
  const [x, y] = [parse(a), parse(b)];
  const out = x.map((v, i) => Math.round(v + (y[i] - v) * Math.max(0, Math.min(1, t))));
  return `#${out.map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}
