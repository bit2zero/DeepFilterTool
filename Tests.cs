// Copyright (c) 2026 bit2zero
// MIT License. See LICENSE and NOTICE.md in the repository root.
//
// WaveData の単体テスト。
//
// .NET SDK も NuGet も使えない環境（Windows 同梱の csc.exe のみ）で動かすため、
// xUnit 相当の最小限の仕組みだけを自前で持つ。属性の名前と意味、テストごとに
// 新しいインスタンスを作る点、Fact / Theory の区別は xUnit に合わせてある。
//
//   ビルド: Build-Tests.cmd
//   実行  : Tests.exe          （すべて実行）
//           Tests.exe Read_     （名前に Read_ を含むものだけ）
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;

// ---------------------------------------------------------------- 最小の枠組み

[AttributeUsage(AttributeTargets.Method)]
public sealed class FactAttribute : Attribute { }

/// 引数付きのテスト。InlineData を必要な数だけ添える。
[AttributeUsage(AttributeTargets.Method)]
public sealed class TheoryAttribute : Attribute { }

[AttributeUsage(AttributeTargets.Method, AllowMultiple = true)]
public sealed class InlineDataAttribute : Attribute {
    public readonly object[] Values;
    public InlineDataAttribute(params object[] values) { Values = values; }
}

public sealed class AssertionException : Exception {
    public AssertionException(string message) : base(message) { }
}

public static class Assert {
    public static void True(bool condition, string because) {
        if (!condition) throw new AssertionException(because);
    }

    public static void False(bool condition, string because) {
        if (condition) throw new AssertionException(because);
    }

    public static void Equal<T>(T expected, T actual, string because) {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
            throw new AssertionException(string.Format(
                "{0}\n      期待: {1}\n      実際: {2}", because, expected, actual));
    }

    public static void Bytes(byte[] expected, byte[] actual, string because) {
        if (expected.Length != actual.Length)
            throw new AssertionException(string.Format(
                "{0}\n      長さが違う 期待 {1} / 実際 {2}", because, expected.Length, actual.Length));
        for (int i = 0; i < expected.Length; i++)
            if (expected[i] != actual[i])
                throw new AssertionException(string.Format(
                    "{0}\n      {1} バイト目が違う 期待 0x{2:X2} / 実際 0x{3:X2}",
                    because, i, expected[i], actual[i]));
    }

    /// 例外が投げられ、かかつメッセージに contains を含むこと。
    public static Exception Throws(Action action, string contains, string because) {
        try {
            action();
        } catch (Exception ex) {
            if (contains != null && ex.Message.IndexOf(contains, StringComparison.Ordinal) < 0)
                throw new AssertionException(string.Format(
                    "{0}\n      期待するメッセージ: 「{1}」を含む\n      実際: {2}", because, contains, ex.Message));
            return ex;
        }
        throw new AssertionException(because + "\n      例外が投げられなかった");
    }
}

// ------------------------------------------------------------ テストデータ生成

/// 検査用の WAV を組み立てる。既定は 48 kHz モノラル PCM 16bit。
public sealed class WaveBuilder {
    ushort format = 1, channels = 1, bits = 16;
    uint rate = 48000;
    int frames = 8;
    ushort alignOverride;
    readonly List<KeyValuePair<string, byte[]>> extraChunks = new List<KeyValuePair<string, byte[]>>();
    byte[] dataOverride;
    long riffSizeOverride = -1;
    bool omitFmt, omitData;

    public WaveBuilder Stereo() { channels = 2; return this; }
    public WaveBuilder Channels(ushort value) { channels = value; return this; }
    public WaveBuilder Float32() { format = 3; bits = 32; return this; }
    public WaveBuilder Format(ushort value) { format = value; return this; }
    public WaveBuilder Bits(ushort value) { bits = value; return this; }
    public WaveBuilder Rate(uint value) { rate = value; return this; }
    public WaveBuilder Frames(int value) { frames = value; return this; }
    public WaveBuilder Align(ushort value) { alignOverride = value; return this; }
    public WaveBuilder Data(byte[] value) { dataOverride = value; return this; }
    public WaveBuilder RiffSize(long value) { riffSizeOverride = value; return this; }
    public WaveBuilder WithoutFmt() { omitFmt = true; return this; }
    public WaveBuilder WithoutData() { omitData = true; return this; }
    public WaveBuilder WithChunk(string id, byte[] body) {
        extraChunks.Add(new KeyValuePair<string, byte[]>(id, body));
        return this;
    }

    ushort Align_ { get { return alignOverride != 0 ? alignOverride : (ushort)(channels * (bits / 8)); } }

    public byte[] Body {
        get {
            if (dataOverride != null) return dataOverride;
            var bytes = new byte[frames * Align_];
            for (int i = 0; i < bytes.Length; i++) bytes[i] = (byte)(i % 251);
            return bytes;
        }
    }

    /// メモリ上に RIFF/WAVE のバイト列を組み立てる。
    public byte[] ToBytes() {
        byte[] body = Body;
        var chunks = new MemoryStream();
        var w = new BinaryWriter(chunks);
        if (!omitFmt) {
            w.Write(Encoding.ASCII.GetBytes("fmt "));
            w.Write(16u);
            w.Write(format); w.Write(channels); w.Write(rate);
            w.Write(rate * Align_); w.Write(Align_); w.Write(bits);
        }
        foreach (var chunk in extraChunks) {
            w.Write(Encoding.ASCII.GetBytes(chunk.Key));
            w.Write((uint)chunk.Value.Length);
            w.Write(chunk.Value);
            if (chunk.Value.Length % 2 == 1) w.Write((byte)0);
        }
        if (!omitData) {
            w.Write(Encoding.ASCII.GetBytes("data"));
            w.Write((uint)body.Length);
            w.Write(body);
            if (body.Length % 2 == 1) w.Write((byte)0);
        }
        w.Flush();
        byte[] payload = chunks.ToArray();

        var file = new MemoryStream();
        var f = new BinaryWriter(file);
        f.Write(Encoding.ASCII.GetBytes("RIFF"));
        f.Write(riffSizeOverride >= 0 ? (uint)riffSizeOverride : (uint)(4 + payload.Length));
        f.Write(Encoding.ASCII.GetBytes("WAVE"));
        f.Write(payload);
        f.Flush();
        return file.ToArray();
    }

    /// 組み立てたバイト列をファイルに落とす。
    public string Save(string path) {
        File.WriteAllBytes(path, ToBytes());
        return path;
    }

    /// WaveData オブジェクトを直接作る（ファイルを介さない検査用）。
    public WaveData Build() {
        return new WaveData {
            Format = format, Channels = channels, Rate = rate,
            Bits = bits, Align = Align_, Data = Body
        };
    }
}

/// テストごとに使い捨てる一時フォルダー。
public sealed class Scratch : IDisposable {
    public readonly string Dir;
    public Scratch() {
        Dir = Path.Combine(Path.GetTempPath(),
            "DeepFilterTool.Tests." + Guid.NewGuid().ToString("N").Substring(0, 8));
        Directory.CreateDirectory(Dir);
    }
    public string Path_(string name) { return Path.Combine(Dir, name); }
    public void Dispose() { try { Directory.Delete(Dir, true); } catch (IOException) { } }
}

// -------------------------------------------------------------- Read のテスト

public sealed class WaveDataReadTests : IDisposable {
    readonly Scratch scratch = new Scratch();
    public void Dispose() { scratch.Dispose(); }

    [Fact]
    public void Read_ReturnsSamples_WhenMonoPcm16() {
        // Arrange
        var builder = new WaveBuilder().Frames(100);
        string path = builder.Save(scratch.Path_("mono.wav"));

        // Act
        var wave = WaveData.Read(path);

        // Assert
        Assert.Equal(1, (int)wave.Channels, "モノラルとして読める");
        Assert.Equal(48000, (int)wave.Rate, "サンプルレートを保つ");
        Assert.Equal(16, (int)wave.Bits, "ビット深度を保つ");
        Assert.Equal(2, (int)wave.Align, "ブロックアラインを保つ");
        Assert.Equal(100, wave.Frames, "フレーム数を数えられる");
        Assert.Bytes(builder.Body, wave.Data, "音声データがそのまま読める");
    }

    [Fact]
    public void Read_ReturnsSamples_WhenStereoPcm16() {
        var builder = new WaveBuilder().Stereo().Frames(64);
        var wave = WaveData.Read(builder.Save(scratch.Path_("stereo.wav")));

        Assert.Equal(2, (int)wave.Channels, "ステレオとして読める");
        Assert.Equal(4, (int)wave.Align, "1 フレーム 4 バイト");
        Assert.Equal(64, wave.Frames, "フレーム数を数えられる");
    }

    [Fact]
    public void Read_ReturnsFloatFormat_WhenIeeeFloat32() {
        var wave = WaveData.Read(new WaveBuilder().Float32().Frames(32)
            .Save(scratch.Path_("float.wav")));

        Assert.Equal(3, (int)wave.Format, "IEEE Float として読める");
        Assert.Equal(32, (int)wave.Bits, "32 bit として読める");
        Assert.Equal(32, wave.Frames, "フレーム数を数えられる");
    }

    [Fact]
    public void Read_SkipsUnknownChunks_WhenTheyPrecedeData() {
        // LIST や fact のような未知のチャンクが挟まっても読み飛ばせること。
        var builder = new WaveBuilder().Frames(16)
            .WithChunk("LIST", Encoding.ASCII.GetBytes("INFO"))
            .WithChunk("fact", new byte[] { 1, 0, 0, 0 });
        var wave = WaveData.Read(builder.Save(scratch.Path_("chunks.wav")));

        Assert.Equal(16, wave.Frames, "未知チャンクを読み飛ばして data に到達する");
        Assert.Bytes(builder.Body, wave.Data, "音声データが壊れない");
    }

    [Fact]
    public void Read_SkipsPaddingByte_WhenChunkLengthIsOdd() {
        // RIFF は奇数長チャンクの後ろに詰め物を 1 バイト置く。
        var builder = new WaveBuilder().Frames(16)
            .WithChunk("odd ", new byte[] { 9, 9, 9 });
        var wave = WaveData.Read(builder.Save(scratch.Path_("odd.wav")));

        Assert.Equal(16, wave.Frames, "詰め物を跨いで data を読める");
    }

    [Fact]
    public void Read_Succeeds_WhenPathContainsJapaneseAndSpaces() {
        var builder = new WaveBuilder().Frames(48);
        string path = builder.Save(scratch.Path_("日本語 & テスト (1).wav"));

        var wave = WaveData.Read(path);

        Assert.Equal(48, wave.Frames, "日本語と空白を含むパスを扱える");
    }

    [Fact]
    public void Read_Throws_WhenNotRiff() {
        string path = scratch.Path_("notwav.bin");
        File.WriteAllBytes(path, Encoding.ASCII.GetBytes("This is not a wave file"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "RIFF WAV を選んでください", "RIFF でないファイルを断る");
    }

    [Fact]
    public void Read_Throws_WhenFileIsEmpty() {
        string path = scratch.Path_("empty.bin");
        File.WriteAllBytes(path, new byte[0]);

        Assert.Throws(delegate { WaveData.Read(path); },
            "RIFF WAV を選んでください", "空ファイルを断る");
    }

    [Theory]
    [InlineData(1)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(7)]
    [InlineData(8)]
    [InlineData(11)]
    public void Read_Throws_WhenHeaderIsTruncated(int length) {
        // 12 バイトの先頭ヘッダーが途中で切れている場合。分割して読むと
        // BinaryReader の内部例外がそのまま利用者に出てしまうため、
        // アプリ自身の案内が出ることを固定する。
        string path = scratch.Path_("truncated" + length + ".bin");
        byte[] full = Encoding.ASCII.GetBytes("RIFF\0\0\0\0WAVE");
        File.WriteAllBytes(path, full.Take(length).ToArray());

        Exception ex = Assert.Throws(delegate { WaveData.Read(path); },
            "RIFF WAV を選んでください",
            string.Format("{0} バイトで切れたファイルを案内付きで断る", length));
        Assert.False(ex is EndOfStreamException, "内部例外を素通しにしない");
    }

    [Fact]
    public void Read_Throws_WhenRiffSizeExceedsFile() {
        string path = new WaveBuilder().Frames(16).RiffSize(9999999)
            .Save(scratch.Path_("bigriff.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "WAV ヘッダーが壊れています", "申告サイズが実体より大きいものを断る");
    }

    [Fact]
    public void Read_Throws_WhenChunkOverrunsRiff() {
        // data チャンクの申告長を、RIFF に収まらない値へ書き換える。
        var bytes = new WaveBuilder().Frames(8).ToBytes();
        int dataLengthAt = bytes.Length - 16 - 4;
        BitConverter.GetBytes(9000u).CopyTo(bytes, dataLengthAt);
        string path = scratch.Path_("overrun.wav");
        File.WriteAllBytes(path, bytes);

        Assert.Throws(delegate { WaveData.Read(path); },
            "WAV が不正", "RIFF をはみ出すチャンクを断る");
    }

    [Fact]
    public void Read_Throws_WhenFmtChunkTooShort() {
        var builder = new WaveBuilder().Frames(8).WithoutFmt()
            .WithChunk("fmt ", new byte[] { 1, 0, 1, 0 });
        string path = builder.Save(scratch.Path_("shortfmt.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "fmt チャンクが不正", "16 バイト未満の fmt を断る");
    }

    [Fact]
    public void Read_Throws_WhenFmtChunkMissing() {
        string path = new WaveBuilder().Frames(8).WithoutFmt()
            .Save(scratch.Path_("nofmt.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "音声データがありません", "fmt がないものを断る");
    }

    [Fact]
    public void Read_Throws_WhenDataChunkMissing() {
        string path = new WaveBuilder().WithoutData().Save(scratch.Path_("nodata.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "音声データがありません", "data がないものを断る");
    }

    [Fact]
    public void Read_Throws_WhenDataChunkEmpty() {
        string path = new WaveBuilder().Data(new byte[0]).Save(scratch.Path_("emptydata.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "音声データがありません", "空の data を断る");
    }

    [Theory]
    [InlineData(8000u)]
    [InlineData(16000u)]
    [InlineData(44100u)]
    [InlineData(96000u)]
    public void Read_Throws_WhenSampleRateIsNot48kHz(uint rate) {
        string path = new WaveBuilder().Rate(rate).Frames(16)
            .Save(scratch.Path_("rate" + rate + ".wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "48 kHz", string.Format("{0} Hz を断る", rate));
    }

    [Theory]
    [InlineData((ushort)0)]
    [InlineData((ushort)3)]
    [InlineData((ushort)6)]
    public void Read_Throws_WhenChannelCountUnsupported(ushort channels) {
        var builder = new WaveBuilder().Channels(channels).Frames(16);
        if (channels == 0) builder.Align(2).Data(new byte[32]);
        string path = builder.Save(scratch.Path_("ch" + channels + ".wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "モノラル/ステレオ", string.Format("{0} ch を断る", channels));
    }

    [Theory]
    [InlineData((ushort)1, (ushort)8)]
    [InlineData((ushort)1, (ushort)24)]
    [InlineData((ushort)1, (ushort)32)]
    [InlineData((ushort)3, (ushort)16)]
    [InlineData((ushort)3, (ushort)64)]
    [InlineData((ushort)2, (ushort)16)]
    [InlineData((ushort)65534, (ushort)16)]
    public void Read_Throws_WhenFormatAndBitDepthUnsupported(ushort format, ushort bits) {
        string path = new WaveBuilder().Format(format).Bits(bits).Frames(16)
            .Save(scratch.Path_(string.Format("fmt{0}b{1}.wav", format, bits)));

        Assert.Throws(delegate { WaveData.Read(path); },
            "PCM 16bit または Float 32bit",
            string.Format("format={0} bits={1} を断る", format, bits));
    }

    [Fact]
    public void Read_Throws_WhenBlockAlignDisagreesWithFormat() {
        // ステレオ 16bit なら 4 バイトのはずのところを 2 と申告する。
        string path = new WaveBuilder().Stereo().Align(2).Frames(16)
            .Save(scratch.Path_("badalign.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "48 kHz", "ブロックアラインの矛盾を断る");
    }

    [Fact]
    public void Read_Throws_WhenDataIsNotWholeFrames() {
        // ステレオ 16bit の 1 フレームは 4 バイト。6 バイトは端数になる。
        string path = new WaveBuilder().Stereo().Data(new byte[6])
            .Save(scratch.Path_("ragged.wav"));

        Assert.Throws(delegate { WaveData.Read(path); },
            "48 kHz", "フレーム境界に揃わない data を断る");
    }
}

// ------------------------------------------------------------- Write のテスト

public sealed class WaveDataWriteTests : IDisposable {
    readonly Scratch scratch = new Scratch();
    public void Dispose() { scratch.Dispose(); }

    [Fact]
    public void Write_RoundTrips_WhenFramesMatchContent() {
        var source = new WaveBuilder().Frames(256).Build();
        string path = scratch.Path_("roundtrip.wav");

        source.Write(path, source.Frames, false);
        var back = WaveData.Read(path);

        Assert.Equal(source.Frames, back.Frames, "フレーム数が一致する");
        Assert.Equal(source.Channels, back.Channels, "チャンネル数が一致する");
        Assert.Equal(source.Rate, back.Rate, "サンプルレートが一致する");
        Assert.Bytes(source.Data, back.Data, "音声データが一致する");
    }

    [Fact]
    public void Write_ProducesCanonical44ByteHeader() {
        // 出力の互換性を固定する。ヘッダーが崩れると再生できない機器が出る。
        var source = new WaveBuilder().Frames(10).Build();
        string path = scratch.Path_("header.wav");

        source.Write(path, 10, false);
        byte[] bytes = File.ReadAllBytes(path);

        Assert.Equal(44 + 20, bytes.Length, "44 バイトのヘッダー + 本体");
        Assert.Equal("RIFF", Encoding.ASCII.GetString(bytes, 0, 4), "RIFF で始まる");
        Assert.Equal(36u + 20u, BitConverter.ToUInt32(bytes, 4), "RIFF サイズは 36 + データ長");
        Assert.Equal("WAVEfmt ", Encoding.ASCII.GetString(bytes, 8, 8), "WAVEfmt が続く");
        Assert.Equal(16u, BitConverter.ToUInt32(bytes, 16), "fmt チャンクは 16 バイト");
        Assert.Equal((ushort)1, BitConverter.ToUInt16(bytes, 20), "PCM 形式");
        Assert.Equal(48000u, BitConverter.ToUInt32(bytes, 24), "48 kHz");
        Assert.Equal(96000u, BitConverter.ToUInt32(bytes, 28), "バイトレートは rate * align");
        Assert.Equal("data", Encoding.ASCII.GetString(bytes, 36, 4), "data チャンクが続く");
        Assert.Equal(20u, BitConverter.ToUInt32(bytes, 40), "データ長が入る");
    }

    [Fact]
    public void Write_PadsWithSilence_WhenFramesExceedContent() {
        // エンジンの先読み分を吐き出させるための無音パディング。
        var source = new WaveBuilder().Frames(100).Build();
        string path = scratch.Path_("padded.wav");

        source.Write(path, 580, true);
        var back = WaveData.Read(path);

        Assert.Equal(580, back.Frames, "指定した長さになる");
        Assert.Bytes(source.Data, back.Data.Take(200).ToArray(), "元の音声はそのまま");
        Assert.True(back.Data.Skip(200).All(delegate(byte b) { return b == 0; }),
            "追加部分は無音である");
    }

    [Fact]
    public void Write_CropsToRequestedLength_WhenFramesBelowContent() {
        // 遅延補正後に元の長さへ切り詰める工程。
        var source = new WaveBuilder().Frames(500).Build();
        string path = scratch.Path_("cropped.wav");

        source.Write(path, 300, false);
        var back = WaveData.Read(path);

        Assert.Equal(300, back.Frames, "指定した長さに切り詰める");
        Assert.Bytes(source.Data.Take(600).ToArray(), back.Data, "先頭から切り出す");
    }

    [Fact]
    public void Write_Throws_WhenFileAlreadyExists() {
        var source = new WaveBuilder().Frames(10).Build();
        string path = scratch.Path_("existing.wav");
        source.Write(path, 10, false);
        byte[] before = File.ReadAllBytes(path);

        Assert.Throws(delegate { source.Write(path, 10, false); }, null,
            "既存ファイルを上書きしない");
        Assert.Bytes(before, File.ReadAllBytes(path), "既存ファイルの中身が変わらない");
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    [InlineData(-100)]
    public void Write_Throws_WhenFrameCountBelowOne(int frames) {
        var source = new WaveBuilder().Frames(10).Build();
        string path = scratch.Path_("zero" + frames + ".wav");

        Assert.Throws(delegate { source.Write(path, frames, true); },
            "処理結果の長さが不足", string.Format("{0} フレームを断る", frames));
        Assert.False(File.Exists(path), "断ったときはファイルを作らない");
    }

    [Fact]
    public void Write_Throws_WhenCroppingBeyondContent() {
        var source = new WaveBuilder().Frames(10).Build();
        string path = scratch.Path_("toolong.wav");

        Assert.Throws(delegate { source.Write(path, 11, false); },
            "処理結果の長さが不足", "パディングなしで長さが足りない場合を断る");
        Assert.False(File.Exists(path), "断ったときはファイルを作らない");
    }

    [Fact]
    public void Write_Succeeds_WhenPathContainsJapaneseAndSpaces() {
        var source = new WaveBuilder().Stereo().Frames(32).Build();
        string path = scratch.Path_("出力 & 結果.wav");

        source.Write(path, 32, false);

        Assert.True(File.Exists(path), "日本語と空白を含むパスに書ける");
        Assert.Equal(32, WaveData.Read(path).Frames, "書いたものを読み戻せる");
    }
}

// ---------------------------------------------------------- ToPcm16 のテスト

public sealed class WaveDataConversionTests {
    static WaveData Float(params float[] samples) {
        var data = new byte[samples.Length * 4];
        for (int i = 0; i < samples.Length; i++)
            BitConverter.GetBytes(samples[i]).CopyTo(data, i * 4);
        return new WaveData {
            Format = 3, Channels = 1, Rate = 48000, Bits = 32, Align = 4, Data = data
        };
    }

    static short[] Samples(WaveData wave) {
        var result = new short[wave.Data.Length / 2];
        for (int i = 0; i < result.Length; i++)
            result[i] = BitConverter.ToInt16(wave.Data, i * 2);
        return result;
    }

    [Fact]
    public void ToPcm16_IsNoOp_WhenAlreadyPcm16() {
        var wave = new WaveBuilder().Frames(10).Build();
        byte[] before = (byte[])wave.Data.Clone();

        wave.ToPcm16();

        Assert.Bytes(before, wave.Data, "PCM 16bit はそのまま通す");
        Assert.Equal(1, (int)wave.Format, "形式も変えない");
    }

    [Fact]
    public void ToPcm16_UpdatesFormatFields_WhenConvertingFloat() {
        var wave = Float(0f, 0.5f, -0.5f);
        wave.Channels = 1;

        wave.ToPcm16();

        Assert.Equal(1, (int)wave.Format, "PCM になる");
        Assert.Equal(16, (int)wave.Bits, "16 bit になる");
        Assert.Equal(2, (int)wave.Align, "ブロックアラインが更新される");
        Assert.Equal(3, wave.Frames, "フレーム数は変わらない");
    }

    [Fact]
    public void ToPcm16_HalvesDataLength_WhenConvertingStereoFloat() {
        var wave = Float(0f, 0f, 0f, 0f);
        wave.Channels = 2; wave.Align = 8;

        wave.ToPcm16();

        Assert.Equal(4, (int)wave.Align, "ステレオ 16bit は 4 バイト");
        Assert.Equal(8, wave.Data.Length, "データ長が半分になる");
        Assert.Equal(2, wave.Frames, "フレーム数は変わらない");
    }

    [Theory]
    [InlineData(0f, (short)0)]
    [InlineData(1f, (short)32767)]
    [InlineData(-1f, (short)-32767)]
    [InlineData(2f, (short)32767)]
    [InlineData(-2f, (short)-32767)]
    [InlineData(1000f, (short)32767)]
    [InlineData(-1000f, (short)-32767)]
    public void ToPcm16_ClampsToFullScale_WhenSampleOutOfRange(float input, short expected) {
        var wave = Float(input);

        wave.ToPcm16();

        Assert.Equal(expected, Samples(wave)[0],
            string.Format("{0} を {1} に丸める", input, expected));
    }

    [Fact]
    public void ToPcm16_UsesBankersRounding_WhenExactlyHalf() {
        // 0.5 * 32767 = 16383.5 → 最近接偶数丸めで 16384。
        // Rust 版もここを揃えてあるので、両実装のバイト列が一致する。
        var wave = Float(0.5f, -0.5f);

        wave.ToPcm16();
        short[] got = Samples(wave);

        Assert.Equal((short)16384, got[0], "0.5 は 16384 に丸める");
        Assert.Equal((short)-16384, got[1], "-0.5 は -16384 に丸める");
    }

    [Theory]
    [InlineData(float.NaN)]
    [InlineData(float.PositiveInfinity)]
    [InlineData(float.NegativeInfinity)]
    public void ToPcm16_Throws_WhenSampleIsNotFinite(float bad) {
        var wave = Float(0f, bad, 0f);

        Assert.Throws(delegate { wave.ToPcm16(); },
            "不正なサンプル", string.Format("{0} を断る", bad));
    }
}

// -------------------------------------------------------------------- 実行部

public static class TestRunner {
    public static int Main(string[] args) {
        Console.OutputEncoding = Encoding.UTF8;
        string filter = args.Length > 0 ? args[0] : null;

        var classes = Assembly.GetExecutingAssembly().GetTypes()
            .Where(delegate(Type t) {
                return t.IsClass && !t.IsAbstract && t.Name.EndsWith("Tests", StringComparison.Ordinal);
            })
            .OrderBy(delegate(Type t) { return t.Name; })
            .ToList();

        int passed = 0, failed = 0, skipped = 0;
        var failures = new List<string>();

        foreach (Type type in classes) {
            Console.WriteLine();
            Console.WriteLine("== " + type.Name + " ==");
            foreach (MethodInfo method in type.GetMethods(BindingFlags.Public | BindingFlags.Instance)
                                              .OrderBy(delegate(MethodInfo m) { return m.MetadataToken; })) {
                bool isFact = method.GetCustomAttributes(typeof(FactAttribute), false).Length > 0;
                var inlines = (InlineDataAttribute[])method.GetCustomAttributes(typeof(InlineDataAttribute), false);
                bool isTheory = method.GetCustomAttributes(typeof(TheoryAttribute), false).Length > 0;
                if (!isFact && !isTheory) continue;
                if (filter != null && method.Name.IndexOf(filter, StringComparison.OrdinalIgnoreCase) < 0) {
                    skipped++;
                    continue;
                }

                if (isTheory) {
                    // InlineData は宣言順に並べ直す（属性の取得順は保証されない）。
                    foreach (InlineDataAttribute inline in inlines.Reverse()) {
                        string label = method.Name + "(" +
                            string.Join(", ", inline.Values.Select(delegate(object v) {
                                return v == null ? "null" : v.ToString();
                            }).ToArray()) + ")";
                        Run(type, method, inline.Values, label, ref passed, ref failed, failures);
                    }
                } else {
                    Run(type, method, null, method.Name, ref passed, ref failed, failures);
                }
            }
        }

        Console.WriteLine();
        Console.WriteLine(new string('-', 60));
        Console.WriteLine(string.Format("成功 {0} / 失敗 {1}{2}", passed, failed,
            skipped > 0 ? string.Format(" / 絞り込みで除外 {0}", skipped) : ""));
        if (failed > 0) {
            Console.WriteLine();
            Console.WriteLine("失敗一覧:");
            foreach (string name in failures) Console.WriteLine("  " + name);
        }
        return failed == 0 ? 0 : 1;
    }

    /// テストごとに新しいインスタンスを作る（xUnit と同じ約束）。
    static void Run(Type type, MethodInfo method, object[] arguments, string label,
                    ref int passed, ref int failed, List<string> failures) {
        object instance = null;
        try {
            instance = Activator.CreateInstance(type);
            method.Invoke(instance, arguments);
            Console.WriteLine("  PASS " + label);
            passed++;
        } catch (Exception ex) {
            Exception real = ex is TargetInvocationException && ex.InnerException != null
                ? ex.InnerException : ex;
            Console.WriteLine("  FAIL " + label);
            foreach (string line in real.Message.Split('\n'))
                Console.WriteLine("       " + line.TrimEnd());
            if (!(real is AssertionException))
                Console.WriteLine("       (" + real.GetType().Name + ")");
            failed++;
            failures.Add(label);
        } finally {
            var disposable = instance as IDisposable;
            if (disposable != null) disposable.Dispose();
        }
    }
}
