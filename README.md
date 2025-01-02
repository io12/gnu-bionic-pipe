# GNU Bionic Pipe

A way for GNU ABI programs to use Android GPU drivers

![logo](logo.png)

> [!WARNING]
> This doesn't work right now,
> and I'm probably not going to work on it again in the future,
> but it still may be useful for someone.

# What it does

Android GPU drivers are compiled for Android's Bionic C library, which is incompatible with the standard Linux GNU C library (glibc).
This project tries to allow the GNU programs to use the Bionic GPU drivers.
There are a few reasons why you would want to do this:
1. Hardware-accelerated graphics in GNU+Linux environments on Android, such as [proot](https://github.com/termux/proot-distro), [chroot](https://wiki.debian.org/ChrootOnAndroid), or even running [glibc programs directly](https://github.com/termux-pacman/glibc-packages)
2. Hardware-accelerated graphics in mobile Linux distributions such as [Mobian](https://en.wikipedia.org/wiki/Mobian) or [postmarketOS](https://en.wikipedia.org/wiki/PostmarketOS) when running on Android devices
3. Hardware-accelerated graphics in Android ports of Linux games and emulators, such as [PojavLauncher](https://pojavlauncherteam.github.io/) and [Winlator](https://winlator.org/)

# Build instructions
1. Install [Nix with flake support](https://zero-to-nix.com/concepts/nix-installer/)

    ``` sh
    curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
    ```
2. In the repo, run
   ```sh
   nix build
   ```
   This will take a long time, since it will also build cross compiler toolchains.
3. Copy the output `result/lib/libgnubionicpipe.so`

# Usage instructions

``` sh
LD_PRELOAD=path/to/libgnubionicpipe.so \
    LIBGNUBIONICPIPE_BIONIC_END_PATH=/data/path/to/store/bionic-end \
    ./executable
```

Here, `path/to/libgnubionicpipe.so` is the path to the built library and
`/data/path/to/store/bionic-end` is a path inside `/data` to
a path to automatically save a file for the bionic end of the pipe.

You can also set `LIBGNUBIONICPIPE_TRACE=1` to trace function calls for debugging.

# How it works

For GNU code to call the Vulkan API functions in the Bionic GPU driver,
both the GNU and Bionic code need to be in the same address space.
This is a problem,
because the only official way to load Bionic code is through `/system/bin/linker64`,
Bionic's runtime dynamic linker program.
The libhybris project solves this by implementing its own custom version of Bionic's linker that can load Bionic libraries from GNU code,
but it relies on internal implementation details of Bionic,
and therefore can break with newer Android versions which
change those implementation details.
Gnu Bionic Pipe tries to load Bionic libraries using the device's built-in `/system/bin/linker64`.
It gets it to run in the same address space by re-implementing the Linux kernel's process loading and execution instead,
which _is_ stable across Linux and Android releases.

Specifically, a Bionic program takes a memory address as an argument,
loads Vulkan, then puts pointers to all the Vulkan functions in a table at that address.
When the GNU library is loaded, it runs this program in the same address space with userland-execve,
passing it the address of a table for the function pointers.
The GNU library defines functions for the entire Vulkan API,
that wrap calls to the corresponding function in the table.
There is also some machinery to avoid conflicts with thread-local variables between glibc and Bionic,
but that is the basic idea.

# Comparison with related tools

| Tool                                                               | Approach                                                                                     | Pros                              | Cons                                                                                                                             |
|--------------------------------------------------------------------|----------------------------------------------------------------------------------------------|-----------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| [VirGL](https://docs.mesa3d.org/drivers/virgl.html)                | Send OpenGL calls from the glibc program to a Bionic server program through a network socket | Works on all GPUs                 | Slow and doesn't support all GPU features                                                                                        |
| [Freedreno/Turnip](https://docs.mesa3d.org/drivers/freedreno.html) | Open source Adreno GPU driver                                                                | Fast                              | Requires a supported Adreno GPU                                                                                                  |
| [Panfrost](https://docs.mesa3d.org/drivers/panfrost.html)          | Open source Mali GPU driver                                                                  | Fast                              | Requires a supported Mali GPU                                                                                                    |
| [libhybris](https://en.wikipedia.org/wiki/Libhybris)               | Re-implement Bionic linker                                                                   | Fast                              | Requires [system patches](https://github.com/mer-hybris/hybris-patches) and doesn't automatically support newer Android versions |
| GNU Bionic Pipe                                                    | See ["how it works"](#how-it-works) section                                                  | Fast, in theory works on all GPUs | Doesn't currently work                                                                                                           |
