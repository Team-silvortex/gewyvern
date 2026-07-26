using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

internal static class RuntimeDeletionCommandIdentity
{
    internal static string ForIntent(string intentId)
    {
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(intentId));
        return $"runtime-unregister-{Convert.ToHexString(digest.AsSpan(0, 16)).ToLowerInvariant()}";
    }
}
