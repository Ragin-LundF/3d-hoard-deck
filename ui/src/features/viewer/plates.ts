/** Decodes the shell's plate binary: `u32 plates`, then per plate `u32 number`, `u32 floats`, `f32 × floats`. */
export function decodePlates(buffer: ArrayBuffer): { number: number; positions: Float32Array }[] {
  const view = new DataView(buffer);
  const plates: { number: number; positions: Float32Array }[] = [];
  let offset = 0;
  const count = view.getUint32(offset, true);
  offset += 4;
  for (let index = 0; index < count; index += 1) {
    const number = view.getUint32(offset, true);
    const floats = view.getUint32(offset + 4, true);
    offset += 8;
    // slice() copies into a fresh, aligned buffer; floats are little-endian like every platform the app runs on.
    const positions = new Float32Array(buffer.slice(offset, offset + floats * 4));
    offset += floats * 4;
    plates.push({ number, positions });
  }
  return plates;
}
