/**
 * Alarme sonore : bips générés localement (Web Audio, aucun fichier,
 * aucun réseau). Le son suit l'état diffusé par le moteur (`sounding`) :
 * il ne s'arrête que par acquittement ou résolution.
 */

/** Sous-ensemble de Web Audio utilisé (injectable pour les tests). */
export interface AudioOut {
  readonly currentTime: number;
  readonly state: string;
  resume(): Promise<void>;
  createOscillator(): {
    type: string;
    frequency: { value: number };
    connect(node: unknown): unknown;
    start(when: number): void;
    stop(when: number): void;
  };
  createGain(): {
    gain: { value: number; setValueAtTime(v: number, t: number): unknown };
    connect(node: unknown): unknown;
  };
  readonly destination: unknown;
}

/** Cadence des salves de bips (ms). */
export const BEEP_PERIOD_MS = 1_000;

/** Salve : trois bips de 150 ms, 880 Hz. */
const BEEPS = [0, 0.25, 0.5];
const BEEP_S = 0.15;

function defaultAudio(): AudioOut | null {
  const Ctor = (globalThis as { AudioContext?: new () => AudioOut }).AudioContext;
  return Ctor ? new Ctor() : null;
}

export class AlarmSound {
  private audio: AudioOut | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  /** Le son n'a pas pu être joué (aucune sortie audio). */
  unavailable = false;

  constructor(private readonly createAudio: () => AudioOut | null = defaultAudio) {}

  get playing(): boolean {
    return this.timer !== null;
  }

  private burst(): void {
    const audio = this.audio;
    if (!audio) return;
    if (audio.state === 'suspended') void audio.resume().catch(() => {});
    const t0 = audio.currentTime;
    for (const offset of BEEPS) {
      const osc = audio.createOscillator();
      const gain = audio.createGain();
      osc.type = 'square';
      osc.frequency.value = 880;
      gain.gain.setValueAtTime(0.25, t0 + offset);
      gain.gain.setValueAtTime(0, t0 + offset + BEEP_S);
      osc.connect(gain);
      gain.connect(audio.destination);
      osc.start(t0 + offset);
      osc.stop(t0 + offset + BEEP_S);
    }
  }

  /** Démarre ou arrête le son selon l'état du moteur. */
  set(sounding: boolean): void {
    if (sounding === this.playing) return;
    if (!sounding) {
      if (this.timer !== null) clearInterval(this.timer);
      this.timer = null;
      return;
    }
    try {
      this.audio ??= this.createAudio();
    } catch {
      this.audio = null;
    }
    this.unavailable = this.audio === null;
    this.burst();
    this.timer = setInterval(() => this.burst(), BEEP_PERIOD_MS);
  }

  /** Arrête le son (démontage). */
  dispose(): void {
    this.set(false);
  }
}
