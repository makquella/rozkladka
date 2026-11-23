import { loadFont } from "@remotion/google-fonts/Manrope";
import { measureText } from "@remotion/layout-utils";
import React, { useEffect, useState } from "react";
import {
  AbsoluteFill,
  Easing,
  Sequence,
  continueRender,
  delayRender,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";

const { fontFamily, waitUntilDone } = loadFont("normal", {
  weights: ["500", "600", "700", "800"],
  subsets: ["latin", "cyrillic"],
});

export const FPS = 30;

// Scenes overlap by a few frames so each hands over to the next.
const S1 = { from: 0, length: 112 };
const S2 = { from: 100, length: 176 };
const S3 = { from: 266, length: 112 };
const S4 = { from: 370, length: 96 };
export const DURATION = S4.from + S4.length;

const PAPER = "#f5f2ec";
const INK = "#1c1b1a";
const MUTED = "#9a968f";
const LINE = "#ece8e1";
const BLUE = "#2f6bff";
const YELLOW = "#f6c945";
const RED = "#e5484d";

const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
const out = Easing.bezier(0.16, 1, 0.3, 1);
const inOut = Easing.bezier(0.65, 0, 0.35, 1);

const ramp = (frame: number, from: number, length: number, easing = out) =>
  interpolate(frame, [from, from + length], [0, 1], { ...clamp, easing });
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

/* ------------------------------------------------------------------ shared pieces */

const Caption: React.FC<{ text: string; at: number; until?: number; top?: number }> = ({
  text,
  at,
  until,
  top = 78,
}) => {
  const frame = useCurrentFrame();
  const show = ramp(frame, at, 16);
  const hide = until === undefined ? 0 : ramp(frame, until, 10, inOut);
  return (
    <div
      style={{
        position: "absolute",
        top,
        width: "100%",
        textAlign: "center",
        fontFamily,
        fontSize: 40,
        fontWeight: 800,
        letterSpacing: -1,
        color: INK,
        opacity: show * (1 - hide),
        transform: `translateY(${(1 - show) * 10}px)`,
      }}
    >
      {text}
    </div>
  );
};

/** A wavy spell-check underline that draws itself from left to right. */
const Squiggle: React.FC<{ width: number; draw: number; opacity?: number; stroke?: number }> = ({
  width,
  draw,
  opacity = 1,
  stroke = 2,
}) => {
  const waves = Math.max(1, Math.round(width / 10));
  let d = "M0 4";
  for (let i = 0; i < waves; i++) d += ` q2.5 -4 5 0 t5 0`;
  return (
    <svg width={width} height={8} viewBox={`0 0 ${waves * 10} 8`} preserveAspectRatio="none" style={{ opacity, display: "block" }}>
      <path
        d={d}
        stroke={RED}
        strokeWidth={stroke}
        fill="none"
        pathLength={1}
        strokeDasharray={1}
        strokeDashoffset={1 - draw}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
};

/* ------------------------------------------------------------------ chat window */

type Bubble = { text: string; at: number; mine?: boolean };

const BubbleView: React.FC<Bubble> = ({ text, at, mine }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const pop = spring({ frame: frame - at, fps, config: { damping: 15, stiffness: 220, mass: 0.6 } });
  if (frame < at) return null;
  return (
    <div
      style={{
        alignSelf: mine ? "flex-end" : "flex-start",
        maxWidth: 420,
        padding: "11px 16px",
        borderRadius: 18,
        borderBottomLeftRadius: mine ? 18 : 6,
        borderBottomRightRadius: mine ? 6 : 18,
        background: mine ? BLUE : "#f1eee8",
        color: mine ? "white" : INK,
        fontSize: 21,
        fontWeight: 500,
        transformOrigin: mine ? "100% 100%" : "0% 100%",
        transform: `scale(${0.85 + 0.15 * pop})`,
        opacity: Math.min(1, pop * 1.4),
      }}
    >
      {text}
    </div>
  );
};

const TypingDots: React.FC<{ from: number; to: number }> = ({ from, to }) => {
  const frame = useCurrentFrame();
  if (frame < from || frame >= to) return null;
  const show = ramp(frame, from, 8);
  return (
    <div
      style={{
        alignSelf: "flex-start",
        display: "flex",
        gap: 6,
        padding: "16px 18px",
        borderRadius: 18,
        borderBottomLeftRadius: 6,
        background: "#f1eee8",
        opacity: show,
      }}
    >
      {[0, 1, 2].map((i) => (
        <div
          key={i}
          style={{
            width: 8,
            height: 8,
            borderRadius: 4,
            background: MUTED,
            transform: `translateY(${Math.sin((frame - from) / 3.2 - i * 0.9) * 3}px)`,
          }}
        />
      ))}
    </div>
  );
};

const SendIcon: React.FC = () => (
  <svg width="20" height="20" viewBox="0 0 20 20">
    <path d="M10 16 V4 M5 9 L10 4 L15 9" stroke="white" strokeWidth="2.2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const WINDOW = { left: 280, top: 160, width: 720, height: 460 };
/** Where the composer text starts, relative to the window. */
const COMPOSER_TEXT = { x: 40, y: WINDOW.height - 39 };

const ChatWindow: React.FC<{
  bubbles: Bubble[];
  typing?: [number, number];
  composer: React.ReactNode;
  sendPressAt?: number;
  style?: React.CSSProperties;
}> = ({ bubbles, typing, composer, sendPressAt, style }) => {
  const frame = useCurrentFrame();
  const pressed = sendPressAt !== undefined && frame >= sendPressAt && frame < sendPressAt + 5;
  return (
    <div
      style={{
        position: "absolute",
        ...WINDOW,
        borderRadius: 24,
        background: "white",
        boxShadow: "0 1px 2px rgba(28,27,26,0.06), 0 18px 50px rgba(28,27,26,0.10)",
        overflow: "hidden",
        fontFamily,
        ...style,
      }}
    >
      <div
        style={{
          height: 70,
          display: "flex",
          alignItems: "center",
          gap: 14,
          padding: "0 24px",
          borderBottom: `1px solid ${LINE}`,
        }}
      >
        <div
          style={{
            width: 40,
            height: 40,
            borderRadius: 20,
            background: YELLOW,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 18,
            fontWeight: 800,
            color: INK,
          }}
        >
          О
        </div>
        <div>
          <div style={{ fontSize: 19, fontWeight: 700, color: INK }}>Оля</div>
          <div style={{ fontSize: 14, fontWeight: 500, color: MUTED }}>в мережі</div>
        </div>
      </div>
      <div
        style={{
          position: "absolute",
          top: 70,
          bottom: 78,
          left: 0,
          right: 0,
          padding: "22px 24px",
          display: "flex",
          flexDirection: "column",
          justifyContent: "flex-end",
          gap: 10,
        }}
      >
        {bubbles.map((bubble) => (
          <BubbleView key={bubble.text} {...bubble} />
        ))}
        {typing && <TypingDots from={typing[0]} to={typing[1]} />}
      </div>
      <div style={{ position: "absolute", bottom: 0, left: 0, right: 0, height: 78, borderTop: `1px solid ${LINE}` }}>
        <div
          style={{
            position: "absolute",
            left: 20,
            right: 80,
            top: 15,
            height: 48,
            borderRadius: 14,
            background: "#f6f4f0",
            display: "flex",
            alignItems: "center",
            padding: "0 20px",
            fontSize: 21,
            fontWeight: 500,
            color: INK,
          }}
        >
          {composer}
        </div>
        <div
          style={{
            position: "absolute",
            right: 20,
            top: 17,
            width: 44,
            height: 44,
            borderRadius: 22,
            background: BLUE,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            transform: `scale(${pressed ? 0.88 : 1})`,
          }}
        >
          <SendIcon />
        </div>
      </div>
    </div>
  );
};

const Caret: React.FC<{ solid?: boolean }> = ({ solid }) => {
  const frame = useCurrentFrame();
  const on = solid || Math.floor(frame / 16) % 2 === 0;
  return <span style={{ display: "inline-block", width: 2, height: 24, marginLeft: 2, background: INK, opacity: on ? 1 : 0 }} />;
};

/* ------------------------------------------------------------------ scene 1: the mistake */

const TYPED = "dljvf";
const MEANT = "вдома";

const SceneMistake: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const typeAt = 26;
  const perChar = 5;
  const count = Math.max(0, Math.min(TYPED.length, Math.floor((frame - typeAt) / perChar) + 1));
  const typing = frame >= typeAt && count < TYPED.length;
  const enter = spring({ frame, fps, config: { damping: 20, stiffness: 120 } });
  // Push in on the typed word and carry it towards where scene two shows it large.
  const zoom = ramp(frame, 76, 34, inOut);
  const scale = lerp(1, 3.4, zoom);
  const origin = { x: WINDOW.left + COMPOSER_TEXT.x + 30, y: WINDOW.top + COMPOSER_TEXT.y };
  const dx = (640 - origin.x) * zoom;
  const dy = (250 - origin.y) * zoom;
  const fade = 1 - ramp(frame, 96, 14, inOut);
  const textWidth = measureText({ text: TYPED, fontFamily, fontSize: 21, fontWeight: "500" }).width;
  return (
    <AbsoluteFill>
      <Caption text="Знайомо?" at={58} until={78} />
      <ChatWindow
        bubbles={[{ text: "ти де?", at: 8 }]}
        composer={
          frame < typeAt ? (
            <span style={{ color: MUTED }}>Повідомлення</span>
          ) : (
            <span style={{ position: "relative", display: "inline-flex", alignItems: "center" }}>
              <span>{TYPED.slice(0, count)}</span>
              <Caret solid={typing} />
              <span style={{ position: "absolute", left: 0, top: 30 }}>
                <Squiggle width={textWidth} draw={ramp(frame, 56, 14, inOut)} stroke={1.6} />
              </span>
            </span>
          )
        }
        style={{
          opacity: Math.min(1, enter * 1.2) * fade,
          transformOrigin: `${origin.x - WINDOW.left}px ${origin.y - WINDOW.top}px`,
          transform: `translate(${dx}px, ${dy + (1 - enter) * 16}px) scale(${scale * (0.98 + 0.02 * enter)})`,
        }}
      />
    </AbsoluteFill>
  );
};

/* ------------------------------------------------------------------ scene 2: same keys */

const KEY = 104;
const GAP = 18;
const WORD_Y = 250;
/** Where the finished word settles: the visual center once the keys are gone. */
const FINAL_Y = 330;
const KEY_TOP = 420;
const BIG = 104;

type Layout = { tightTyped: number[]; tightMeant: number[]; meantWidth: number; typedWidth: number };

/** Glyph centers for a word set at `BIG`, centered on the frame. */
function tight(word: string): { centers: number[]; width: number } {
  const widths = word.split("").map((c) => measureText({ text: c, fontFamily, fontSize: BIG, fontWeight: "700" }).width);
  const width = widths.reduce((a, b) => a + b, 0);
  let x = 640 - width / 2;
  const centers = widths.map((w) => {
    const c = x + w / 2;
    x += w;
    return c;
  });
  return { centers, width };
}

const keyLeft = (i: number) => 640 - (TYPED.length * KEY + (TYPED.length - 1) * GAP) / 2 + i * (KEY + GAP);
const spreadX = (i: number) => keyLeft(i) + KEY / 2;

const Glyph: React.FC<{ char: string; x: number; y: number; size: number; color: string; opacity?: number; weight?: number }> = ({
  char,
  x,
  y,
  size,
  color,
  opacity = 1,
  weight = 700,
}) => (
  <div
    style={{
      position: "absolute",
      left: x,
      top: y,
      transform: "translate(-50%, -50%)",
      fontSize: size,
      fontWeight: weight,
      lineHeight: 1,
      color,
      opacity,
      whiteSpace: "pre",
    }}
  >
    {char}
  </div>
);

/** ENG/УКР switch between the word and the keys. */
const LayoutPill: React.FC<{ at: number; appearAt: number; leaveAt: number }> = ({ at, appearAt, leaveAt }) => {
  const frame = useCurrentFrame();
  const show = ramp(frame, appearAt, 12) * (1 - ramp(frame, leaveAt, 12, inOut));
  const flip = ramp(frame, at, 12, inOut);
  const switched = flip >= 0.5;
  const label = (text: string, offset: number, opacity: number) => (
    <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center", transform: `translateY(${offset}px)`, opacity }}>
      {text}
    </div>
  );
  return (
    <div
      style={{
        position: "absolute",
        left: 640 - 62,
        top: PILL_Y - 24,
        width: 124,
        height: 48,
        borderRadius: 24,
        overflow: "hidden",
        fontSize: 20,
        fontWeight: 800,
        letterSpacing: 1.5,
        color: switched ? "white" : INK,
        background: switched ? BLUE : "white",
        border: `1px solid ${switched ? BLUE : LINE}`,
        opacity: show,
        transform: `scale(${0.9 + 0.1 * show})`,
      }}
    >
      {label("ENG", -flip * 30, 1 - flip)}
      {label("УКР", (1 - flip) * 30, flip)}
    </div>
  );
};

const PILL_Y = 345;

const SceneKeys: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const [layout, setLayout] = useState<Layout | null>(null);
  const [handle] = useState(() => delayRender("measure glyphs"));
  useEffect(() => {
    waitUntilDone().then(() => {
      const a = tight(TYPED);
      const b = tight(MEANT);
      setLayout({ tightTyped: a.centers, tightMeant: b.centers, typedWidth: a.width, meantWidth: b.width });
      continueRender(handle);
    });
  }, [handle]);
  if (!layout) return null;

  // Beats: keys appear, the typed keys are pressed, the layout switches, the same keys are
  // pressed again and each letter becomes its Ukrainian one, then the word settles.
  const pressA = (i: number) => 44 + i * 6;
  const SWITCH = 82;
  const pressB = (i: number) => 104 + i * 6;
  const SETTLE = 140;

  const fadeIn = ramp(frame, 0, 12);
  const fadeOut = 1 - ramp(frame, 166, 10, inOut);
  const spread = ramp(frame, 14, 18, inOut);
  const switched = ramp(frame, SWITCH + 4, 10);
  const keysAway = ramp(frame, SETTLE, 14, inOut);
  const converge = ramp(frame, SETTLE, 18, inOut);

  const keys: React.ReactNode[] = [];
  const glyphs: React.ReactNode[] = [];
  for (let i = 0; i < TYPED.length; i++) {
    const appear = spring({ frame: frame - (16 + i * 3), fps, config: { damping: 16, stiffness: 160 } });
    const pressing = [pressA(i), pressB(i)].some((at) => frame >= at && frame < at + 6);
    const dip = pressing ? 4 : 0;
    keys.push(
      <div
        key={`key${i}`}
        style={{
          position: "absolute",
          left: keyLeft(i),
          top: KEY_TOP + dip + (1 - appear) * 20 + keysAway * 16,
          width: KEY,
          height: KEY,
          boxSizing: "border-box",
          borderRadius: 18,
          background: "white",
          border: `1px solid ${LINE}`,
          borderBottom: `${pressing ? 2 : 6}px solid #e0dbd2`,
          opacity: Math.min(1, appear) * (1 - keysAway),
        }}
      >
        <div
          style={{
            position: "absolute",
            left: 18,
            top: 12,
            fontSize: 28,
            fontWeight: 700,
            color: mix(INK, "#cfcac2", switched),
          }}
        >
          {TYPED[i]}
        </div>
        <div
          style={{
            position: "absolute",
            right: 18,
            bottom: 12,
            fontSize: 30,
            fontWeight: 700,
            color: mix("#c9c4bc", BLUE, switched),
          }}
        >
          {MEANT[i]}
        </div>
      </div>,
    );

    // The letter above its key: Latin until the second press, then Ukrainian.
    const pulse = interpolate(frame, [pressA(i), pressA(i) + 3, pressA(i) + 10], [0, 1, 0], clamp);
    const out = ramp(frame, pressB(i), 6, inOut);
    const inn = ramp(frame, pressB(i) + 3, 9);
    const baseX = lerp(layout.tightTyped[i], spreadX(i), spread);
    glyphs.push(
      <Glyph
        key={`lat${i}`}
        char={TYPED[i]}
        x={baseX}
        y={WORD_Y - out * 24}
        size={BIG * (1 + 0.08 * pulse)}
        color={mix(INK, BLUE, pulse)}
        opacity={fadeIn * (1 - out)}
      />,
    );
    glyphs.push(
      <Glyph
        key={`cyr${i}`}
        char={MEANT[i]}
        x={lerp(spreadX(i), layout.tightMeant[i], converge)}
        y={lerp(WORD_Y + (1 - inn) * 24, FINAL_Y, converge)}
        size={BIG}
        color={mix(BLUE, INK, converge)}
        opacity={inn}
      />,
    );
  }

  const squiggleFade = 1 - ramp(frame, 12, 8);
  const marker = ramp(frame, SETTLE + 14, 14, inOut);
  return (
    <AbsoluteFill style={{ fontFamily, opacity: fadeOut }}>
      <Caption text="Клавіші правильні" at={38} until={SWITCH - 6} />
      <Caption text="Розкладка не та" at={SWITCH} until={SETTLE + 4} />
      <div style={{ position: "absolute", left: 640 - layout.typedWidth / 2, top: WORD_Y + 52, opacity: fadeIn * squiggleFade }}>
        <Squiggle width={layout.typedWidth} draw={1} stroke={3} />
      </div>
      <svg
        style={{ position: "absolute", left: 640 - layout.meantWidth / 2 - 10, top: FINAL_Y + 36, overflow: "visible" }}
        width={layout.meantWidth + 20}
        height={30}
      >
        <path
          d={`M4 18 Q ${(layout.meantWidth + 20) / 2} 8 ${layout.meantWidth + 16} 16`}
          stroke={YELLOW}
          strokeWidth={16}
          strokeLinecap="round"
          fill="none"
          pathLength={1}
          strokeDasharray={1}
          strokeDashoffset={1 - marker}
        />
      </svg>
      <LayoutPill at={SWITCH} appearAt={36} leaveAt={SETTLE} />
      {keys}
      {glyphs}
    </AbsoluteFill>
  );
};

/* ------------------------------------------------------------------ scene 3: sent */

const SceneSent: React.FC = () => {
  const frame = useCurrentFrame();
  const enter = ramp(frame, 0, 18);
  const leave = ramp(frame, 100, 12, inOut);
  const sendAt = 30;
  return (
    <AbsoluteFill>
      <Caption text="Rozkladka виправляє це сама" at={6} until={100} />
      <ChatWindow
        bubbles={[
          { text: "ти де?", at: -100 },
          { text: MEANT, at: sendAt, mine: true },
          { text: "ок, зайду", at: 78 },
        ]}
        typing={[48, 78]}
        sendPressAt={sendAt - 3}
        composer={
          frame < sendAt ? (
            <span style={{ display: "inline-flex", alignItems: "center" }}>
              {MEANT}
              <Caret />
            </span>
          ) : (
            <span style={{ color: MUTED }}>Повідомлення</span>
          )
        }
        style={{
          opacity: enter * (1 - leave),
          transform: `scale(${lerp(1.06, 1, enter)})`,
        }}
      />
    </AbsoluteFill>
  );
};

/* ------------------------------------------------------------------ scene 4: name */

const KeyLogo: React.FC<{ size: number }> = ({ size }) => (
  <div
    style={{
      position: "relative",
      width: size,
      height: size,
      boxSizing: "border-box",
      borderRadius: size * 0.2,
      background: "white",
      border: `1px solid ${LINE}`,
      borderBottom: `${size * 0.06}px solid #e0dbd2`,
      fontFamily,
    }}
  >
    <div style={{ position: "absolute", left: size * 0.2, top: size * 0.12, fontSize: size * 0.3, fontWeight: 600, color: MUTED }}>
      G
    </div>
    <div
      style={{ position: "absolute", right: size * 0.18, bottom: size * 0.1, fontSize: size * 0.36, fontWeight: 800, color: BLUE }}
    >
      П
    </div>
  </div>
);

const SceneName: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const logo = spring({ frame, fps, config: { damping: 14, stiffness: 140 } });
  const [name, line, link] = [ramp(frame, 8, 18), ramp(frame, 16, 18), ramp(frame, 26, 18)];
  const leave = ramp(frame, 86, 10, inOut);
  return (
    <AbsoluteFill
      style={{ fontFamily, alignItems: "center", justifyContent: "center", flexDirection: "column", opacity: 1 - leave }}
    >
      <div style={{ transform: `translateY(${(1 - logo) * 24}px) rotate(${(1 - logo) * -8}deg)`, opacity: Math.min(1, logo * 1.3) }}>
        <KeyLogo size={112} />
      </div>
      <div
        style={{
          marginTop: 30,
          fontSize: 76,
          fontWeight: 800,
          letterSpacing: -2.5,
          color: INK,
          opacity: name,
          transform: `translateY(${(1 - name) * 12}px)`,
        }}
      >
        Rozkladka
      </div>
      <div
        style={{
          marginTop: 10,
          fontSize: 26,
          fontWeight: 500,
          color: MUTED,
          opacity: line,
          transform: `translateY(${(1 - line) * 10}px)`,
        }}
      >
        Виправляє текст, набраний не в тій розкладці
      </div>
      <div style={{ marginTop: 34, fontSize: 22, fontWeight: 700, color: BLUE, opacity: link }}>
        github.com/makquella/rozkladka
      </div>
    </AbsoluteFill>
  );
};

export const Promo: React.FC = () => (
  <AbsoluteFill style={{ background: PAPER }}>
    <Sequence from={S1.from} durationInFrames={S1.length}>
      <SceneMistake />
    </Sequence>
    <Sequence from={S2.from} durationInFrames={S2.length}>
      <SceneKeys />
    </Sequence>
    <Sequence from={S3.from} durationInFrames={S3.length}>
      <SceneSent />
    </Sequence>
    <Sequence from={S4.from} durationInFrames={S4.length}>
      <SceneName />
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
