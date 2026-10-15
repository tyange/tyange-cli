// 손글씨 사진에서 글씨만 남기고 배경은 투명하게 만든다.
// usage: bun extract-ink.ts <input> <output.png> [x,y,w,h]
import sharp from "sharp";

const [input, output, cropArg] = process.argv.slice(2);
if (!input || !output) throw new Error("usage: bun extract-ink.ts <input> <output.png> [x,y,w,h]");

let img = sharp(input).rotate();
if (cropArg) {
  const [left, top, width, height] = cropArg.split(",").map(Number);
  img = sharp(await img.extract({ left, top, width, height }).toBuffer());
}

const { data, info } = await img.greyscale().raw().toBuffer({ resolveWithObject: true });
const bgBuf = await sharp(data, { raw: { width: info.width, height: info.height, channels: 1 } })
  .blur(35)
  .raw()
  .toBuffer();

// 종이 밝기(배경)와의 비율로 잉크 농도를 구한다. 그림자·조명 얼룩이 같이 사라진다.
const LOW = 0.12;
const HIGH = 0.45;
const out = Buffer.alloc(info.width * info.height * 4);
for (let i = 0; i < data.length; i++) {
  const ratio = Math.min(1, data[i] / Math.max(1, bgBuf[i]));
  const ink = 1 - ratio;
  const a = Math.max(0, Math.min(1, (ink - LOW) / (HIGH - LOW)));
  out[i * 4] = 24;
  out[i * 4 + 1] = 24;
  out[i * 4 + 2] = 28;
  out[i * 4 + 3] = Math.round(a * 255);
}
await sharp(out, { raw: { width: info.width, height: info.height, channels: 4 } })
  .png({ compressionLevel: 9 })
  .toFile(output);
console.log("saved", output, info.width + "x" + info.height);
