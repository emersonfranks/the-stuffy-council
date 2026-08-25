# Local image generation setup

This runbook installs a local-only image-editing workstation for producing
Stuffy Council portraits and scenes from private reference photos. It targets
Windows 11 with an NVIDIA RTX 4070 12 GB, 32 GB or more of system RAM, and at
least 35 GB of free disk space.

The selected stack is ComfyUI `v0.33.1` with the native FLUX.1 Kontext Dev FP8
workflow. Kontext accepts an image and an English edit instruction, preserves
character identity across iterations, and supports both photorealistic and
illustrated output. ComfyUI remains separate from the Rust application; only
approved PNGs enter this repository.

## License and operating boundary

Review and accept the
[FLUX.1 Dev Non-Commercial License](https://huggingface.co/black-forest-labs/FLUX.1-Kontext-dev/blob/main/LICENSE.md)
before downloading weights. The model is for non-commercial use. Use only
reference photos that the operator is authorized to process.
The license requires filtering or manual review; this profile uses manual
review, so do not run unattended or bulk generation and inspect every output.

This profile deliberately excludes:

- ComfyUI Manager and all custom nodes.
- Partner/API nodes and cloud inference.
- LAN exposure, CORS, and automatic browser launch.
- Executable model formats; every model artifact is `safetensors`.

## What gets installed

| Component | Source | Purpose |
| --- | --- | --- |
| ComfyUI `v0.33.1` | official Comfy-Org GitHub repository | local workflow engine and UI |
| Python 3.12 virtual environment | existing Python installation | isolates dependencies |
| Stable PyTorch CUDA build | official PyTorch package index | NVIDIA inference runtime |
| Kontext Dev FP8 | official Comfy-Org repack | reference-preserving image editing |
| CLIP-L and T5XXL FP8 | ComfyUI maintainer repository | prompt encoding |
| FLUX autoencoder | official Comfy-Org repack | image encode/decode |

Allow 35 GB: approximately 17.6 GB for models, 10 GB for ComfyUI and its
virtual environment, and 7.4 GB for temporary and generated images. Installation
and generation make no external inference calls. Initial cloning, package
installation, model download, and deliberate updates require internet access.

## Prerequisites

Install these prerequisites:

- Git for Windows.
- Python 3.12 x64 with the `py` launcher.
- A current NVIDIA Studio Driver or Game Ready Driver.

Run every command below in Git Bash unless it is labeled PowerShell.

Confirm the expected hardware and interpreter:

```bash
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader
py -3.12 --version
git --version
```

Stop if `nvidia-smi` shows less than 12 GB of VRAM. Use the
[official ComfyUI manual-install guide](https://docs.comfy.org/installation/manual_install)
for other hardware instead of improvising package sources.

## Step 1: Install pinned ComfyUI

Keep the runtime outside this repository so model weights and generated files
cannot be committed accidentally.

```bash
mkdir -p "$HOME/Apps"
git clone --branch v0.33.1 --depth 1 \
  https://github.com/Comfy-Org/ComfyUI.git "$HOME/Apps/ComfyUI"
cd "$HOME/Apps/ComfyUI"
test "$(git rev-parse HEAD)" = \
  '72865f4f27eaf5396f8f36370e0a2be3a9a090ee' || exit 1
py -3.12 -m venv .venv
source .venv/Scripts/activate
python -m pip install --upgrade pip
python -m pip install torch torchvision torchaudio \
  --index-url https://download.pytorch.org/whl/cu130
python -m pip install -r requirements.txt
```

**Verify:**

```bash
python -c 'import torch; assert torch.cuda.is_available(); print(torch.__version__, torch.version.cuda, torch.cuda.get_device_name(0))'
git describe --tags --exact-match
```

The first command must print the NVIDIA GPU name; the second must print
`v0.33.1`. The commit check before environment creation must exit successfully.

## Step 2: Download the native Kontext models

Do not use ComfyUI Manager to install models. Download only the four artifacts
named here, from the exact upstream repositories used by ComfyUI's native
Kontext workflow.

```bash
cd "$HOME/Apps/ComfyUI"
mkdir -p models/diffusion_models models/text_encoders models/vae

curl --fail --location --retry 3 \
  --output models/diffusion_models/flux1-dev-kontext_fp8_scaled.safetensors \
  https://huggingface.co/Comfy-Org/flux1-kontext-dev_ComfyUI/resolve/main/split_files/diffusion_models/flux1-dev-kontext_fp8_scaled.safetensors

curl --fail --location --retry 3 \
  --output models/text_encoders/clip_l.safetensors \
  https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/clip_l.safetensors

curl --fail --location --retry 3 \
  --output models/text_encoders/t5xxl_fp8_e4m3fn_scaled.safetensors \
  https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/t5xxl_fp8_e4m3fn_scaled.safetensors

curl --fail --location --retry 3 \
  --output models/vae/ae.safetensors \
  https://huggingface.co/Comfy-Org/Lumina_Image_2.0_Repackaged/resolve/main/split_files/vae/ae.safetensors
```

**Verify:** compare each file with its upstream Git LFS SHA-256 object id.

```bash
cd "$HOME/Apps/ComfyUI"
printf '%s  %s\n' \
  '630ba795ec64283b4230ea23cf79406c2c68b7c578229ed139f30043eadb30a2' \
  'models/diffusion_models/flux1-dev-kontext_fp8_scaled.safetensors' \
  '660c6f5b1abae9dc498ac2d21e1347d2abdb0cf6c0c0c8576cd796491d9a6cdd' \
  'models/text_encoders/clip_l.safetensors' \
  'a498f0485dc9536735258018417c3fd7758dc3bccc0a645feaa472b34955557a' \
  'models/text_encoders/t5xxl_fp8_e4m3fn_scaled.safetensors' \
  'afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38' \
  'models/vae/ae.safetensors' \
  | sha256sum --check --strict -
```

All four lines must end in `OK`. Delete and redownload any file that fails.
Upstream replacement of an artifact requires a fresh review; do not update the
hash merely to make the check pass.

## Step 3: Start the constrained local service

This profile reserves 3 GB of VRAM for Windows and other applications, moves
VAE work to the CPU, avoids retaining node results, disables custom and API
nodes, and binds explicitly to loopback. The 11.9 GB diffusion file plus working
tensors exceeds the approximately 9 GB left for ComfyUI, so dynamic offload to
system RAM is expected and each image will be slower than an unrestricted run.
Generation still uses substantial GPU compute while a job is active, so run it
only when needed. The
[startup-flags reference](https://docs.comfy.org/development/comfyui-server/startup-flags)
and `python main.py --help` on the pinned tag define the controls below.

```bash
cd "$HOME/Apps/ComfyUI"
source .venv/Scripts/activate
python main.py \
  --listen 127.0.0.1 \
  --port 8188 \
  --reserve-vram 3 \
  --cpu-vae \
  --cache-none \
  --disable-all-custom-nodes \
  --disable-api-nodes \
  --disable-auto-launch
```

Open <http://127.0.0.1:8188> manually.

**Verify:** another Git Bash terminal must show loopback, not `0.0.0.0`:

```bash
curl --fail http://127.0.0.1:8188/system_stats
powershell.exe -NoProfile -Command '$listeners = Get-NetTCPConnection -State Listen -LocalPort 8188; $listeners | Format-Table -AutoSize; if (!$listeners -or @($listeners | Where-Object LocalAddress -ne "127.0.0.1").Count -ne 0) { exit 1 }'
```

The PowerShell command must exit successfully and list only
`127.0.0.1:8188`. Any `0.0.0.0:8188` or `[::]:8188` listener is a failure;
stop ComfyUI and correct the launch command.

Stop ComfyUI with `Ctrl+C`. It consumes no GPU compute after the process exits.
If another workload needs more GPU memory, raise `--reserve-vram` to `4`; expect
slower generation. Do not use `--listen` without the explicit address because
that exposes ComfyUI on all interfaces.

## Step 4: Run the first reference edit

1. Open ComfyUI and choose **Workflow Templates**.
2. Search for and load **Flux Kontext Dev**.
3. In **Load Diffusion Model**, select
   `flux1-dev-kontext_fp8_scaled.safetensors`.
4. In **DualCLIP Load**, select `clip_l.safetensors` and
   `t5xxl_fp8_e4m3fn_scaled.safetensors`.
5. In **Load VAE**, select `ae.safetensors`.
6. Load a local reference photo in **Load Image**.
7. Paste all five prompt parts from [character-art.md](character-art.md) in
  order: style key, character identity lock, `Shot:`, `Output:`, and global
  negatives. Keep every part except the `Shot:` line byte-for-byte unchanged.
8. Queue one image with `Ctrl+Enter`.
9. Save the graph as `stuffy-kontext-local` with **Workflow → Save As**.
10. After approving a look, record and pin its seed; vary only the `Shot:` line.

Start at 768×768 for drafts. Generate one image at a time and produce 1024×1024
only for finalists. Kontext supports English prompts; make changes in small
steps and state which identity, pose, composition, or background details must
remain unchanged.

ComfyUI writes results under `$HOME/Apps/ComfyUI/output`. Private references
stay outside the repository. A generated candidate may remain there or be
copied to `art-review/<stable-id>--candidate-<label-slug>.png` for the private
admin review flow; never place an unreviewed candidate under `static/`.

This runbook ends at candidate generation: Kontext output is opaque and is not
a canonical portrait. Alpha cutout, padding, resizing, and metadata removal
require a separate tool not covered here. Promote a candidate to
`static/stuffies/<stable-id>.png` only after it independently satisfies every
asset check in [character-art.md](character-art.md).

## Real-world scene workflow

For a scene where a Council character appears in a real room:

1. Use the real-world photo as the input image.
2. Request one character insertion at a time.
3. Name the placement, scale, lighting direction, contact shadow, and occlusion.
4. Explicitly preserve the room, camera perspective, and existing people.
5. Make subsequent edits from the previous approved output.
6. Add dialogue bubbles afterward according to
   [character-art.md](character-art.md); never ask the model to render text.

Do not use photos of people without their consent. Keep private source photos
outside both this repository and ComfyUI's output directory.

## Offline check

After installation, disconnect networking or block outbound access for the
ComfyUI process, start it with the Step 3 command, load the saved
`stuffy-kontext-local` workflow, and run it. Native Kontext generation must
still complete. A workflow that asks for credentials, downloads another model,
or contains a non-core node is not this local profile.

## Update procedure

Do not track ComfyUI `master`. Review a stable release and its dependency/model
changes first, then update deliberately:

```bash
cd "$HOME/Apps/ComfyUI"
source .venv/Scripts/activate
REVIEWED_TAG=v0.x.y
REVIEWED_COMMIT='replace-with-full-reviewed-commit-sha'
git fetch --tags --prune
git checkout "$REVIEWED_TAG"
test "$(git rev-parse HEAD)" = "$REVIEWED_COMMIT" || exit 1
python -m pip install -r requirements.txt
python main.py --help
```

Reconfirm that every Step 3 flag still exists, rerun the CUDA and hash checks,
and repeat the offline check. Model updates require separate license and hash
review.

## Removal

Stop ComfyUI, then remove its isolated directory. This deletes the virtual
environment, model weights, private inputs, and generated outputs together.

```bash
rm -rf "$HOME/Apps/ComfyUI"
```

No system Python packages or services are installed by this runbook.

## Troubleshooting

### `Torch not compiled with CUDA enabled`

The CPU PyTorch build was selected. With the virtual environment active:

```bash
python -m pip uninstall -y torch torchvision torchaudio
python -m pip install torch torchvision torchaudio \
  --index-url https://download.pytorch.org/whl/cu130
```

### CUDA out of memory

Close other GPU-heavy applications, restart ComfyUI, and retry one 768×768 job.
If contention must remain, change `--reserve-vram 3` to `--reserve-vram 4`.
Do not switch to `--highvram` or reduce the reservation.

### Template or nodes are missing

Confirm `git describe --tags --exact-match` prints `v0.33.1`. Do not install a
custom node to fill the gap. Reinstall the pinned release and its requirements,
or review a newer stable ComfyUI tag before updating.

### The browser cannot connect

Keep the terminal running and confirm the startup log reports
`127.0.0.1:8188`. Port `8188` may already be occupied; identify and stop the
other local process rather than exposing ComfyUI on a different interface.
