import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type KeyboardEvent,
  type SetStateAction,
} from 'react';

export type CopyStatus = 'idle' | 'copied' | 'failed';

async function writeToClipboard(value: string) {
  if (!navigator.clipboard?.writeText) return false;

  let deadlineId: number | undefined;
  const clipboardWrite = navigator.clipboard
    .writeText(value)
    .then(() => true)
    .catch(() => false);
  const deadline = new Promise<boolean>((resolve) => {
    deadlineId = window.setTimeout(() => resolve(false), 400);
  });
  const copied = await Promise.race([clipboardWrite, deadline]);
  if (deadlineId !== undefined) window.clearTimeout(deadlineId);
  return copied;
}

export function useCopyFeedback(resetDelay = 1600) {
  const [copyStatus, setCopyStatus] = useState<CopyStatus>('idle');
  const resetTimer = useRef<number | null>(null);
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (resetTimer.current !== null) {
        window.clearTimeout(resetTimer.current);
      }
    };
  }, []);

  const copyText = async (value: string) => {
    const copied = await writeToClipboard(value);
    if (!mounted.current) return copied;

    setCopyStatus(copied ? 'copied' : 'failed');
    if (resetTimer.current !== null) {
      window.clearTimeout(resetTimer.current);
    }
    resetTimer.current = window.setTimeout(() => {
      setCopyStatus('idle');
      resetTimer.current = null;
    }, resetDelay);
    return copied;
  };

  return { copyStatus, copyText };
}

export function moveTabFocus<T extends string>(
  event: KeyboardEvent<HTMLButtonElement>,
  items: readonly T[],
  index: number,
  setActive: Dispatch<SetStateAction<T>>,
) {
  let nextIndex = index;
  if (event.key === 'ArrowRight') nextIndex = (index + 1) % items.length;
  if (event.key === 'ArrowLeft') {
    nextIndex = (index - 1 + items.length) % items.length;
  }
  if (event.key === 'Home') nextIndex = 0;
  if (event.key === 'End') nextIndex = items.length - 1;
  if (nextIndex === index) return;

  event.preventDefault();
  setActive(items[nextIndex]);
  const tabs =
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
      '[role="tab"]',
    );
  tabs?.[nextIndex]?.focus();
}
