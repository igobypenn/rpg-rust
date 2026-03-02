using System;
using System.Runtime.InteropServices;

namespace MyApp.PInvoke
{
    public static class NativeMethods
    {
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool CloseHandle(IntPtr handle);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr LoadLibrary(string lpFileName);

        [DllImport("mylib.dll")]
        private static extern int add_numbers(int a, int b);

        [DllImport("mylib.dll", EntryPoint = "process_data")]
        private static extern int process_data(IntPtr data, IntPtr len);

        public static int AddViaPInvoke(int a, int b)
        {
            return add_numbers(a, b);
        }
    }

    [ComImport]
    [Guid("000209FF-0000-0000-C000-000000000046")]
    public interface IComInterface
    {
        void DoSomething();
    }

    public static class NativeAotExports
    {
        [UnmanagedCallersOnly(EntryPoint = "my_export")]
        public static int MyExport(int x)
        {
            return x * 2;
        }

        [UnmanagedCallersOnly]
        public static int AnotherExport(int a, int b)
        {
            return a + b;
        }
    }
}
