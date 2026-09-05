using System;
using System.IO;
using System.Reflection;
using System.Threading;
using System.Diagnostics;
using System.Drawing;
using System.Windows.Forms;
public static class Verify {
    static object Get(object o, string name) { return o.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic).GetValue(o); }
    static void Call(object o, string name) { o.GetType().GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic).Invoke(o, null); }
    static void Check(bool ok, string text) { if (!ok) throw new Exception(text); Console.WriteLine("PASS " + text); }
    [STAThread] public static int Main() {
        try {
            string root=AppDomain.CurrentDomain.BaseDirectory;
            string dir=Path.Combine(root,"verification",DateTime.Now.ToString("yyyyMMdd-HHmmss")); Directory.CreateDirectory(dir);
            Application.EnableVisualStyles(); Application.SetCompatibleTextRenderingDefault(false);
            using(var form=new FilterForm()) {
                form.Show(); Application.DoEvents();
                foreach(int channels in new int[]{1,2}) {
                    int frames=48001;
                    var wav=new WaveData { Format=1, Channels=(ushort)channels, Rate=48000, Bits=16, Align=(ushort)(2*channels), Data=new byte[frames*2*channels] };
                    var rng=new Random(42);
                    for(int i=0;i<frames;i++) for(int ch=0;ch<channels;ch++) {
                        short value=(short)(1800*(rng.NextDouble()*2-1)+2500*Math.Sin(i*2*Math.PI*180/48000));
                        byte[] b=BitConverter.GetBytes(value); wav.Data[(i*channels+ch)*2]=b[0];wav.Data[(i*channels+ch)*2+1]=b[1];
                    }
                    string input=Path.Combine(dir,"日本語 & test-"+channels+".wav");wav.Write(input,frames,false);
                    ((TextBox)Get(form,"file")).Text=input;
                    Call(form,"StartFilter");
                    Check(Get(form,"process")!=null,"engine started / channels="+channels);
                    var watch=Stopwatch.StartNew();
                    while(Get(form,"process")!=null && watch.Elapsed.TotalSeconds<40) { Call(form,"Poll"); Application.DoEvents(); Thread.Sleep(40); }
                    Check(Get(form,"process")==null,"engine completed");
                    string output=(string)Get(form,"result");
                    Check(output!=null, "GUI pipeline result: "+((Label)Get(form,"status")).Text);
                    var clean=WaveData.Read(output);
                    Check(clean.Frames==frames && clean.Channels==channels && clean.Rate==48000,"length / channels / sample rate retained");
                    Check(clean.Format==1 && clean.Bits==16,"PCM16 output");
                    Check(Convert.ToBase64String(WaveData.Read(input).Data)==Convert.ToBase64String(wav.Data),"input unchanged");
                    bool changed=false;for(int i=0;i<clean.Data.Length;i++) if(clean.Data[i]!=wav.Data[i]) {changed=true;break;}
                    Check(changed,"model modifies signal");
                    Check(((Button)Get(form,"after")).Enabled && ((Button)Get(form,"save")).Enabled,"result controls enabled");
                }
                using(var bitmap=new Bitmap(form.Width,form.Height)) { form.DrawToBitmap(bitmap,new Rectangle(0,0,bitmap.Width,bitmap.Height));bitmap.Save(Path.Combine(dir,"app-preview.png")); }
                Call(form,"StartFilter");Call(form,"CancelFilter");
                var cancelWatch=Stopwatch.StartNew();
                while(Get(form,"process")!=null && cancelWatch.Elapsed.TotalSeconds<10) { Call(form,"Poll");Thread.Sleep(40); }
                Check(Get(form,"process")==null,"cancel terminates process");
                Check((string)Get(form,"result")==null,"cancel does not publish result");
                Console.WriteLine("Artifacts: "+dir);
            }
            return 0;
        } catch(Exception ex) { Console.Error.WriteLine(ex);return 1; }
    }
}
