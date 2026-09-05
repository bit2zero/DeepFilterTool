using System;
using System.IO;
using System.Text;

public sealed class WaveData {
    public ushort Format, Channels, Bits, Align;
    public uint Rate;
    public byte[] Data;
    public static WaveData Read(string path) {
        using (var r = new BinaryReader(File.OpenRead(path))) {
            if (Encoding.ASCII.GetString(r.ReadBytes(4)) != "RIFF") throw new Exception("RIFF WAV を選んでください。");
            uint riff = r.ReadUInt32();
            if (Encoding.ASCII.GetString(r.ReadBytes(4)) != "WAVE" || (long)riff + 8 > r.BaseStream.Length) throw new Exception("WAV ヘッダーが壊れています。");
            var w = new WaveData(); bool fmt = false;
            long end = (long)riff + 8;
            while (r.BaseStream.Position + 8 <= end) {
                string id = Encoding.ASCII.GetString(r.ReadBytes(4)); uint n = r.ReadUInt32();
                long next = r.BaseStream.Position + n + (n % 2);
                if (next > end || n > 512 * 1024 * 1024) throw new Exception("WAV が不正、または512 MBを超えています。");
                if (id == "fmt ") {
                    if (n < 16) throw new Exception("fmt チャンクが不正です。");
                    w.Format = r.ReadUInt16(); w.Channels = r.ReadUInt16(); w.Rate = r.ReadUInt32();
                    r.ReadUInt32(); w.Align = r.ReadUInt16(); w.Bits = r.ReadUInt16(); fmt = true;
                } else if (id == "data") w.Data = r.ReadBytes((int)n);
                r.BaseStream.Position = next;
            }
            if (!fmt || w.Data == null || w.Data.Length == 0) throw new Exception("音声データがありません。");
            if (w.Rate != 48000 || (w.Channels != 1 && w.Channels != 2) ||
                !((w.Format == 1 && w.Bits == 16) || (w.Format == 3 && w.Bits == 32)) ||
                w.Align != w.Channels * (w.Bits / 8) || w.Data.Length % w.Align != 0)
                throw new Exception("48 kHz、モノラル/ステレオ、PCM 16bit または Float 32bit の WAV に対応しています。");
            return w;
        }
    }
    public int Frames { get { return Data.Length / Align; } }
    public void ToPcm16() {
        if (Format == 1) return;
        var pcm = new byte[Data.Length / 2];
        for (int i = 0; i < pcm.Length / 2; i++) {
            float v = BitConverter.ToSingle(Data, i * 4);
            if (Single.IsNaN(v) || Single.IsInfinity(v)) throw new Exception("処理結果に不正なサンプルがあります。");
            short sample = (short)Math.Round(Math.Max(-1.0, Math.Min(1.0, v)) * 32767);
            byte[] b = BitConverter.GetBytes(sample); pcm[i * 2] = b[0]; pcm[i * 2 + 1] = b[1];
        }
        Data = pcm; Format = 1; Bits = 16; Align = (ushort)(Channels * 2);
    }
    public void Write(string path, int frames, bool pad) {
        if (frames < 1 || (!pad && frames > Frames)) throw new Exception("処理結果の長さが不足しています。");
        int count = checked(frames * Align);
        using (var w = new BinaryWriter(new FileStream(path, FileMode.CreateNew, FileAccess.Write))) {
            w.Write(Encoding.ASCII.GetBytes("RIFF")); w.Write(checked((uint)(36 + count)));
            w.Write(Encoding.ASCII.GetBytes("WAVEfmt ")); w.Write(16u); w.Write(Format); w.Write(Channels);
            w.Write(Rate); w.Write(Rate * Align); w.Write(Align); w.Write(Bits);
            w.Write(Encoding.ASCII.GetBytes("data")); w.Write(count);
            w.Write(Data, 0, Math.Min(Data.Length, count));
            if (count > Data.Length) w.Write(new byte[count - Data.Length]);
        }
    }
}
