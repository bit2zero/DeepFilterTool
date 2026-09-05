//! 48 kHz WAV の読み書き。Windows 版 AudioCore.cs と同じ検証規則・同じ出力バイト列。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{Context, Error, Result};

const MAX_CHUNK_BYTES: u32 = 512 * 1024 * 1024;

pub struct Wave {
    pub format: u16,
    pub channels: u16,
    pub rate: u32,
    pub align: u16,
    pub bits: u16,
    pub data: Vec<u8>,
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Wave {
    pub fn frames(&self) -> usize {
        self.data.len() / self.align as usize
    }

    pub fn read(path: &Path) -> Result<Wave> {
        let mut file = File::open(path).context(format!("WAV を開けません: {}", path.display()))?;
        let size = file.metadata()?.len();

        let mut head = [0u8; 12];
        file.read_exact(&mut head)
            .map_err(|_| Error::new("RIFF WAV を選んでください。"))?;
        if &head[0..4] != b"RIFF" {
            return Err(Error::new("RIFF WAV を選んでください。"));
        }
        let riff = u32le(&head, 4) as u64;
        if &head[8..12] != b"WAVE" || riff + 8 > size {
            return Err(Error::new("WAV ヘッダーが壊れています。"));
        }

        let end = riff + 8;
        let mut format = 0u16;
        let mut channels = 0u16;
        let mut rate = 0u32;
        let mut align = 0u16;
        let mut bits = 0u16;
        let mut data: Option<Vec<u8>> = None;
        let mut has_fmt = false;

        let mut pos = 12u64;
        while pos + 8 <= end {
            file.seek(SeekFrom::Start(pos))?;
            let mut header = [0u8; 8];
            if file.read_exact(&mut header).is_err() {
                break;
            }
            let id = [header[0], header[1], header[2], header[3]];
            let n = u32le(&header, 4);
            let body = pos + 8;
            let next = body + n as u64 + (n as u64 % 2);
            if next > end || n > MAX_CHUNK_BYTES {
                return Err(Error::new("WAV が不正、または512 MBを超えています。"));
            }
            if &id == b"fmt " {
                if n < 16 {
                    return Err(Error::new("fmt チャンクが不正です。"));
                }
                let mut fmt = [0u8; 16];
                file.read_exact(&mut fmt)
                    .map_err(|_| Error::new("fmt チャンクが不正です。"))?;
                format = u16le(&fmt, 0);
                channels = u16le(&fmt, 2);
                rate = u32le(&fmt, 4);
                align = u16le(&fmt, 12);
                bits = u16le(&fmt, 14);
                has_fmt = true;
            } else if &id == b"data" {
                let mut body_bytes = vec![0u8; n as usize];
                file.read_exact(&mut body_bytes)
                    .map_err(|_| Error::new("音声データが途中で切れています。"))?;
                data = Some(body_bytes);
            }
            pos = next;
        }

        let data = data.unwrap_or_default();
        if !has_fmt || data.is_empty() {
            return Err(Error::new("音声データがありません。"));
        }
        let wave = Wave {
            format,
            channels,
            rate,
            align,
            bits,
            data,
        };
        if wave.rate != 48000
            || (wave.channels != 1 && wave.channels != 2)
            || !((wave.format == 1 && wave.bits == 16) || (wave.format == 3 && wave.bits == 32))
            || wave.align != wave.channels * (wave.bits / 8)
            || wave.data.len() % wave.align as usize != 0
        {
            return Err(Error::new(
                "48 kHz、モノラル/ステレオ、PCM 16bit または Float 32bit の WAV に対応しています。",
            ));
        }
        Ok(wave)
    }

    /// IEEE Float 32bit を PCM 16bit へその場で変換する。既に PCM 16bit なら何もしない。
    pub fn convert_to_pcm16(&mut self) -> Result<()> {
        if self.format == 1 {
            return Ok(());
        }
        let count = self.data.len() / 4;
        let mut out = vec![0u8; count * 2];
        for i in 0..count {
            let value = f32::from_le_bytes([
                self.data[i * 4],
                self.data[i * 4 + 1],
                self.data[i * 4 + 2],
                self.data[i * 4 + 3],
            ]);
            if value.is_nan() || value.is_infinite() {
                return Err(Error::new("処理結果に不正なサンプルがあります。"));
            }
            // C# の Math.Round と同じ「最近接偶数」丸めで、既存版と同じバイト列にする。
            let scaled = (value as f64).clamp(-1.0, 1.0) * 32767.0;
            let sample = scaled.round_ties_even() as i16;
            let bytes = sample.to_le_bytes();
            out[i * 2] = bytes[0];
            out[i * 2 + 1] = bytes[1];
        }
        self.data = out;
        self.format = 1;
        self.bits = 16;
        self.align = self.channels * 2;
        Ok(())
    }

    /// frames フレーム分を書き出す。pad=true なら不足分を無音で埋める。既存ファイルは上書きしない。
    pub fn write(&self, path: &Path, frames: usize, pad: bool) -> Result<()> {
        if frames < 1 || (!pad && frames > self.frames()) {
            return Err(Error::new("処理結果の長さが不足しています。"));
        }
        let count = frames
            .checked_mul(self.align as usize)
            .ok_or_else(|| Error::new("WAV のサイズ計算があふれました。"))?;
        if count + 36 > u32::MAX as usize {
            return Err(Error::new("WAV が 4 GB を超えるため書き出せません。"));
        }

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .context(format!(
                "書き出せません（既存ファイルは上書きしません）: {}",
                path.display()
            ))?;
        let mut out = std::io::BufWriter::new(file);
        out.write_all(b"RIFF")?;
        out.write_all(&((36 + count) as u32).to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&16u32.to_le_bytes())?;
        out.write_all(&self.format.to_le_bytes())?;
        out.write_all(&self.channels.to_le_bytes())?;
        out.write_all(&self.rate.to_le_bytes())?;
        out.write_all(&(self.rate * self.align as u32).to_le_bytes())?;
        out.write_all(&self.align.to_le_bytes())?;
        out.write_all(&self.bits.to_le_bytes())?;
        out.write_all(b"data")?;
        out.write_all(&(count as u32).to_le_bytes())?;
        let body = &self.data[..count.min(self.data.len())];
        out.write_all(body)?;
        if count > body.len() {
            let mut remaining = count - body.len();
            let zeros = [0u8; 8192];
            while remaining > 0 {
                let step = remaining.min(zeros.len());
                out.write_all(&zeros[..step])?;
                remaining -= step;
            }
        }
        out.flush()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "wave_tests.rs"]
mod tests;
