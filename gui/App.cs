// Copyright (c) 2026 bit2zero
// MIT License. See LICENSE and NOTICE.md in the repository root.
using System;
using System.IO;
using System.Drawing;
using System.Diagnostics;
using System.Threading.Tasks;
using System.Windows.Forms;

public sealed class FilterForm : Form {
    readonly string root = AppDomain.CurrentDomain.BaseDirectory;
    readonly TextBox file = new TextBox { ReadOnly = true, Width = 650 };
    readonly Label status = new Label { AutoSize = false, Width = 670, Height = 70 };
    readonly Button choose = new Button { Text = "WAVを選択", Width = 150 };
    readonly Button run = new Button { Text = "ノイズを除去", Width = 150 };
    readonly Button cancel = new Button { Text = "中止", Width = 100, Enabled = false };
    readonly Button before = new Button { Text = "元の音声を再生", Width = 150 };
    readonly Button after = new Button { Text = "処理後を再生", Width = 150, Enabled = false };
    readonly Button save = new Button { Text = "名前を付けて保存", Width = 170, Enabled = false };
    readonly CheckBox pf = new CheckBox { Text = "強めの除去（ポストフィルター）", AutoSize = true };
    readonly NumericUpDown strength = new NumericUpDown { Minimum = 1, Maximum = 100, Value = 100, Width = 75 };
    readonly ProgressBar progress = new ProgressBar { Width = 650, Height = 8 };
    readonly System.Windows.Forms.Timer timer = new System.Windows.Forms.Timer { Interval = 200 };
    System.Media.SoundPlayer player;
    Process process; Task<string> stdout, stderr;
    string result, session; int inputFrames; bool cancelled;
    public FilterForm() {
        Text = "DeepFilter • 音声ノイズ除去"; ClientSize = new Size(730, 480);
        MinimumSize = new Size(750, 520); Font = new Font("Yu Gothic UI", 10);
        BackColor = Color.FromArgb(245, 247, 250); StartPosition = FormStartPosition.CenterScreen;
        var stack = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = new Padding(26), AutoScroll = true };
        Controls.Add(stack);
        stack.Controls.Add(new Label { Text = "音声を、もっとクリアに。", Font = new Font(Font.FontFamily, 22, FontStyle.Bold), AutoSize = true, Margin = new Padding(0, 0, 0, 10) });
        stack.Controls.Add(new Label { Text = "DeepFilterNet3  /  ローカル処理  /  48 kHz WAV", AutoSize = true, Margin = new Padding(0, 0, 0, 20) });
        stack.Controls.Add(file); stack.Controls.Add(choose);
        var settings = new FlowLayoutPanel { Width = 670, Height = 40 };
        settings.Controls.Add(new Label { Text = "最大ノイズ抑制 (dB)", AutoSize = true }); settings.Controls.Add(strength); settings.Controls.Add(pf); stack.Controls.Add(settings);
        var actions = new FlowLayoutPanel { Width = 670, Height = 42 }; actions.Controls.Add(run); actions.Controls.Add(cancel); stack.Controls.Add(actions);
        stack.Controls.Add(progress); stack.Controls.Add(status);
        var listen = new FlowLayoutPanel { Width = 670, Height = 42 }; listen.Controls.Add(before); listen.Controls.Add(after); listen.Controls.Add(save); stack.Controls.Add(listen);
        var stop = new Button { Text = "再生停止", Width = 100 }; stack.Controls.Add(stop);
        choose.Click += delegate { using (var d = new OpenFileDialog { Filter = "WAV 音声|*.wav", CheckFileExists = true }) { if (d.ShowDialog() == DialogResult.OK) { file.Text = d.FileName; result = null; after.Enabled = save.Enabled = false; status.Text = "準備できました。"; } } };
        run.Click += delegate { StartFilter(); };
        cancel.Click += delegate { CancelFilter(); };
        before.Click += delegate { Play(file.Text); }; after.Click += delegate { Play(result); }; stop.Click += delegate { StopAudio(); };
        save.Click += delegate { SaveResult(); }; timer.Tick += delegate { Poll(); };
        FormClosing += delegate(object sender, FormClosingEventArgs e) { if (process != null) { e.Cancel = true; status.Text = "処理中です。「中止」してから閉じてください。"; } else StopAudio(); };
        status.Text = File.Exists(Engine) && File.Exists(Model) ? "WAVファイルを選択してください。" : "エンジン未導入です。README の導入対象を確認してください。";
    }
    string Engine { get { return Path.Combine(root, "runtime", "deep-filter.exe"); } }
    string Model { get { return Path.Combine(root, "runtime", "DeepFilterNet3_onnx.tar.gz"); } }
    static string Quote(string s) { if (s.Contains("\"") || s.EndsWith("\\")) throw new Exception("パスの形式が不正です。"); return "\"" + s + "\""; }
    void Busy(bool value) { choose.Enabled = run.Enabled = strength.Enabled = pf.Enabled = !value; cancel.Enabled = value; progress.Style = value ? ProgressBarStyle.Marquee : ProgressBarStyle.Blocks; }
    void StopAudio() { if (player != null) { player.Stop(); player.Dispose(); player = null; } }
    void CancelFilter() {
        if (process == null) return;
        try {
            if (process.HasExited) return;
            process.Kill(); cancelled = true; cancel.Enabled = false;
            status.Text = "処理の終了を待っています…";
        } catch (InvalidOperationException) {
            // The process may finish between HasExited and Kill; let Poll collect its result.
        } catch (Exception ex) {
            status.Text = "中止できませんでした。処理の終了を待ってください: " + ex.Message;
        }
    }
    void Play(string path) { try { StopAudio(); if (String.IsNullOrEmpty(path)) return; player = new System.Media.SoundPlayer(path); player.Load(); player.Play(); } catch (Exception ex) { status.Text = "再生できません: " + ex.Message; } }
    void StartFilter() {
        try {
            if (!File.Exists(Engine) || !File.Exists(Model)) throw new Exception("公式エンジンとモデルが未導入です。README を確認してください。");
            var w = WaveData.Read(file.Text); inputFrames = w.Frames; StopAudio();
            session = Path.Combine(root, "sessions", DateTime.Now.ToString("yyyyMMdd-HHmmss") + "-" + Guid.NewGuid().ToString("N").Substring(0, 8));
            Directory.CreateDirectory(session); string input = Path.Combine(session, "input.wav");
            // Padding flushes model lookahead and incomplete final hops; crop only after delay compensation.
            w.Write(input, checked(((inputFrames + 479) / 480) * 480 + 4800), true);
            string output = Path.Combine(session, "filtered");
            var si = new ProcessStartInfo(Engine, "-m " + Quote(Model) + " -D -a " + strength.Value.ToString(System.Globalization.CultureInfo.InvariantCulture) + (pf.Checked ? " --pf" : "") + " -o " + Quote(output) + " " + Quote(input));
            si.UseShellExecute = false; si.CreateNoWindow = true; si.RedirectStandardError = si.RedirectStandardOutput = true; si.WorkingDirectory = session;
            process = new Process { StartInfo = si }; process.Start();
            stdout = process.StandardOutput.ReadToEndAsync(); stderr = process.StandardError.ReadToEndAsync();
            cancelled = false; result = null; after.Enabled = save.Enabled = false; Busy(true); timer.Start();
            status.Text = "ノイズ除去中… 元ファイルは保持されます。";
        } catch (Exception ex) { if (process != null) { process.Dispose(); process = null; } Busy(false); status.Text = ex.Message; }
    }
    void Poll() {
        if (process == null || !process.HasExited || !stdout.IsCompleted || !stderr.IsCompleted) return;
        timer.Stop();
        try {
            string log = stdout.Result + Environment.NewLine + stderr.Result;
            File.WriteAllText(Path.Combine(session, "engine.log"), log);
            if (cancelled) { status.Text = "中止しました。途中のファイルは sessions に残ります。"; return; }
            if (process.ExitCode != 0) throw new Exception("エンジン終了コード " + process.ExitCode + "。詳細: " + Path.Combine(session, "engine.log"));
            var w = WaveData.Read(Path.Combine(session, "filtered", "input.wav"));
            w.ToPcm16();
            string dest = Path.Combine(session, "clean.wav");
            w.Write(dest, inputFrames, false); result = dest; after.Enabled = save.Enabled = true;
            status.Text = "完了しました。処理前後を聴き比べて保存できます。\n" + result;
        } catch (Exception ex) { status.Text = "処理失敗: " + ex.Message; }
        finally { process.Dispose(); process = null; Busy(false); }
    }
    void SaveResult() {
        try {
            using (var d = new SaveFileDialog { Filter = "WAV 音声|*.wav", FileName = Path.GetFileNameWithoutExtension(file.Text) + "_clean.wav", OverwritePrompt = true }) {
                if (d.ShowDialog() != DialogResult.OK) return;
                if (String.Equals(Path.GetFullPath(d.FileName), Path.GetFullPath(file.Text), StringComparison.OrdinalIgnoreCase)) throw new Exception("元ファイルとは別の名前を指定してください。");
                File.Copy(result, d.FileName, true); status.Text = "保存しました: " + d.FileName;
            }
        } catch (Exception ex) { status.Text = "保存失敗: " + ex.Message; }
    }
    [STAThread] public static void Main() { Application.EnableVisualStyles(); Application.SetCompatibleTextRenderingDefault(false); Application.Run(new FilterForm()); }
}
