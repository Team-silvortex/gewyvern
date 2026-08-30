# Tutorials

Use this shelf when you want to learn by reaching a working end state. Each
tutorial names its prerequisites, side effects, success checkpoint, and stop
conditions. Reference pages explain every option; tutorials deliberately do
not.

## Start By Outcome

| Outcome | Tutorial | Side effects |
| --- | --- | --- |
| Run Gewyvern and read one diagnosis | [First real run](tutorial-first-run.md) | Builds locally and writes optional reports under `/tmp` |
| Learn the native Hub without an account | [First Leserpent Desktop session](tutorial-leserpent-desktop.md) | Starts an app-owned local daemon; remote mutation is optional |
| Author and compile one `.gewy` package | [First GewyLang package](tutorial-gewylang-package.md) | Writes a disposable package under `/tmp` |
| Prove GUI, CLI, and Leselang equivalence | [First Leselang GUI automation](tutorial-leselang-gui-automation.md) | Canonical export is local and non-executing |
| Deploy a disposable remote stack | [Remote deployment lab](tutorial-remote-deployment-lab.md) | Installs and later retires remote services |

## Recommended Order

1. Complete [First real run](tutorial-first-run.md).
2. Open [First Leserpent Desktop session](tutorial-leserpent-desktop.md).
3. Choose the language path that matches your work:
   [GewyLang package](tutorial-gewylang-package.md) for protocol behavior, or
   [Leselang GUI automation](tutorial-leselang-gui-automation.md) for control
   and UI equivalence.
4. Use the [remote deployment lab](tutorial-remote-deployment-lab.md) only on
   disposable Linux targets with a prepared authority and opaque credential
   handles.

The first four tutorials are safe on a developer workstation. The remote lab
is intentionally separate because bootstrap, provisioning, and retirement are
real infrastructure mutations.

## Continue By Role

### Operator

- [First real run](tutorial-first-run.md)
- [First Leserpent Desktop session](tutorial-leserpent-desktop.md)
- [Remote deployment lab](tutorial-remote-deployment-lab.md)
- [Field validation](../field-validation.md)

### Language Author

- [First GewyLang package](tutorial-gewylang-package.md)
- [First Leselang GUI automation](tutorial-leselang-gui-automation.md)
- [GewyLang module](../modules/gewylang.md)
- [Leselang module](../modules/leselang.md)

### Runtime Contributor

- [GewyLang to IR](explanation-gewylang-to-ir.md)
- [Gewy to runtime](explanation-gewy-to-runtime.md)
- [Protocol package spine](explanation-protocol-package-spine.md)
- [Runtime validation](how-to-validate-runtime-surface.md)

## Tutorial Contract

A tutorial is complete only when the reader can state the final observed
checkpoint. If a prerequisite is missing, stop at the named preparation step;
do not replace endpoint trust, explicit confirmation, stable operation IDs, or
opaque credential handles with an undocumented shortcut.
