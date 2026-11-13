import { afterEach, describe, expect, it, vi } from 'vitest';
import { AlarmSound, BEEP_PERIOD_MS, type AudioOut } from './alarmSound';

function fakeAudio(state = 'running') {
  const started: number[] = [];
  let resumed = 0;
  const audio: AudioOut = {
    currentTime: 0,
    state,
    destination: {},
    resume: () => {
      resumed++;
      return Promise.resolve();
    },
    createOscillator: () => ({
      type: 'sine',
      frequency: { value: 0 },
      connect: () => undefined,
      start: (t: number) => started.push(t),
      stop: () => undefined,
    }),
    createGain: () => ({
      gain: { value: 1, setValueAtTime: () => undefined },
      connect: () => undefined,
    }),
  };
  return { audio, started, resumed: () => resumed };
}

describe('AlarmSound', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('bipe tant que le moteur le demande', () => {
    vi.useFakeTimers();
    const { audio, started } = fakeAudio();
    const sound = new AlarmSound(() => audio);
    sound.set(true);
    expect(sound.playing).toBe(true);
    expect(started).toHaveLength(3);
    vi.advanceTimersByTime(BEEP_PERIOD_MS * 2);
    expect(started).toHaveLength(9);
    sound.set(true);
    vi.advanceTimersByTime(BEEP_PERIOD_MS);
    expect(started).toHaveLength(12);
    sound.set(false);
    vi.advanceTimersByTime(BEEP_PERIOD_MS * 5);
    expect(started).toHaveLength(12);
    expect(sound.playing).toBe(false);
  });

  it('relance une sortie audio suspendue', () => {
    vi.useFakeTimers();
    const { audio, resumed } = fakeAudio('suspended');
    const sound = new AlarmSound(() => audio);
    sound.set(true);
    expect(resumed()).toBe(1);
    sound.dispose();
  });

  it('signale une sortie audio absente sans planter', () => {
    vi.useFakeTimers();
    const sound = new AlarmSound(() => {
      throw new Error('pas de carte son');
    });
    sound.set(true);
    expect(sound.unavailable).toBe(true);
    expect(sound.playing).toBe(true);
    sound.dispose();
    expect(sound.playing).toBe(false);
    expect(new AlarmSound(() => null).unavailable).toBe(false);
  });
});
