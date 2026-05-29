import { rustCurseWarns } from '$build/warns'

export const hasWarn = (code: string, includes: string) =>
  rustCurseWarns.some((w) => w.code === code && w.message.includes(includes))
