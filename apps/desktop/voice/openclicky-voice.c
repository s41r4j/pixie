// openclicky-voice: a persistent local neural voice (Kokoro via sherpa-onnx's C API).
//
// The model loads once; each request then takes ~0.25× real time on Apple silicon.
//   openclicky-voice <runtime-dir> <model-dir> [threads]
// stdin, one request per line (tab-separated):
//   id \t sid \t speed \t out.wav \t loudness \t text [\t phonemes]
// `phonemes` (Kokoro's IPA symbols, from Pixie's own lexicon lookup) is spoken when present, so
// words missing from the model's lexicon aren't skipped; `text` is the fallback. `loudness` is
// the target RMS in dBFS (e.g. -16); 0 leaves the level as the model made it.
// stdout:  "ready\t<speakers>\t<sample_rate>"  then per request
//          "<id>\tok\t<samples>\t<sample_rate>"  or  "<id>\terr\t<message>"
//
// libsherpa-onnx-c-api.dylib is loaded at runtime with dlopen, so this builds without the
// library present; the vendored header (Apache-2.0, k2-fsa/sherpa-onnx v2.0.1) fixes the ABI.

#include <dlfcn.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vendor/sherpa-onnx-c-api.h"

typedef const SherpaOnnxOfflineTts *(*create_fn)(const SherpaOnnxOfflineTtsConfig *);
typedef const SherpaOnnxGeneratedAudio *(*generate_fn)(const SherpaOnnxOfflineTts *, const char *, int32_t, float);
typedef const SherpaOnnxGeneratedAudio *(*generate_cfg_fn)(const SherpaOnnxOfflineTts *, const char *,
                                                           const SherpaOnnxGenerationConfig *, void *, void *);
typedef void (*destroy_audio_fn)(const SherpaOnnxGeneratedAudio *);
typedef int32_t (*write_wave_fn)(const float *, int32_t, int32_t, const char *);
typedef int32_t (*int_fn)(const SherpaOnnxOfflineTts *);

// UTF-8 → codepoints, split into sentences of at most `max` codepoints at punctuation or spaces.
static int32_t utf8_codepoints(const char *s, int32_t *out, int32_t cap) {
  int32_t n = 0;
  const unsigned char *p = (const unsigned char *)s;
  while (*p && n < cap) {
    uint32_t c = *p++;
    int extra = c >= 0xF0 ? 3 : c >= 0xE0 ? 2 : c >= 0xC0 ? 1 : 0;
    if (extra) c &= (0x3F >> extra);
    for (int i = 0; i < extra && (*p & 0xC0) == 0x80; i++) c = (c << 6) | (*p++ & 0x3F);
    out[n++] = (int32_t)c;
  }
  return n;
}

static int32_t split_sentences(const int32_t *cp, int32_t n, int32_t max, int32_t *lens, int32_t cap) {
  int32_t count = 0, start = 0;
  while (start < n && count < cap) {
    int32_t end = start + max < n ? start + max : n;
    if (end < n) {
      int32_t cut = -1;
      for (int32_t i = end - 1; i > start + max / 3; i--) {
        if (cp[i] == '.' || cp[i] == ',' || cp[i] == ';' || cp[i] == '!' || cp[i] == '?') { cut = i + 1; break; }
      }
      if (cut < 0) for (int32_t i = end - 1; i > start + max / 3; i--) if (cp[i] == ' ') { cut = i + 1; break; }
      if (cut > 0) end = cut;
    }
    lens[count++] = end - start;
    start = end;
  }
  return count;
}

// Bring speech to a target loudness with a soft limiter so peaks never clip.
static float *level(const float *in, int32_t n, float target_db) {
  float *out = malloc(sizeof(float) * (size_t)n);
  double sum = 0;
  for (int32_t i = 0; i < n; i++) sum += (double)in[i] * in[i];
  double rms = sqrt(sum / (n > 0 ? n : 1));
  float gain = 1.0f;
  if (target_db < 0 && rms > 1e-5) {
    gain = (float)(pow(10.0, target_db / 20.0) / rms);
    if (gain > 8.0f) gain = 8.0f;
    if (gain < 0.25f) gain = 0.25f;
  }
  const float knee = 0.7f;
  for (int32_t i = 0; i < n; i++) {
    float x = in[i] * gain, a = fabsf(x);
    if (a > knee) {
      float over = (a - knee) / (1.0f - knee);
      a = knee + (1.0f - knee) * (over / (1.0f + over)) * 0.98f;
      x = x < 0 ? -a : a;
    }
    out[i] = x;
  }
  return out;
}

static char *join(const char *a, const char *b) {
  size_t n = strlen(a) + strlen(b) + 2;
  char *s = malloc(n);
  snprintf(s, n, "%s/%s", a, b);
  return s;
}

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: openclicky-voice <runtime-dir> <model-dir> [threads]\n");
    return 2;
  }
  setvbuf(stdout, NULL, _IOLBF, 0);
  char *lib = join(argv[1], "lib/libsherpa-onnx-c-api.dylib");
  void *h = dlopen(lib, RTLD_NOW | RTLD_LOCAL);
  if (!h) {
    printf("fatal\t%s\n", dlerror());
    return 1;
  }
  create_fn create = (create_fn)dlsym(h, "SherpaOnnxCreateOfflineTts");
  generate_fn generate = (generate_fn)dlsym(h, "SherpaOnnxOfflineTtsGenerate");
  destroy_audio_fn destroy_audio = (destroy_audio_fn)dlsym(h, "SherpaOnnxDestroyOfflineTtsGeneratedAudio");
  write_wave_fn write_wave = (write_wave_fn)dlsym(h, "SherpaOnnxWriteWave");
  int_fn speakers = (int_fn)dlsym(h, "SherpaOnnxOfflineTtsNumSpeakers");
  int_fn rate = (int_fn)dlsym(h, "SherpaOnnxOfflineTtsSampleRate");
  generate_cfg_fn generate_cfg = (generate_cfg_fn)dlsym(h, "SherpaOnnxOfflineTtsGenerateWithConfig");
  if (!create || !generate || !destroy_audio || !write_wave || !speakers || !rate) {
    printf("fatal\tmissing symbols in %s\n", lib);
    return 1;
  }

  const char *m = argv[2];
  SherpaOnnxOfflineTtsConfig config;
  memset(&config, 0, sizeof(config));
  config.model.kokoro.model = join(m, "model.onnx");
  config.model.kokoro.voices = join(m, "voices.bin");
  config.model.kokoro.tokens = join(m, "tokens.txt");
  config.model.kokoro.lexicon = join(m, "lexicon-us-en.txt");
  config.model.kokoro.length_scale = 1.0f;
  config.model.num_threads = argc > 3 ? atoi(argv[3]) : 4;
  config.model.provider = "cpu";
  config.max_num_sentences = 1;
  config.silence_scale = 0.2f;

  const SherpaOnnxOfflineTts *tts = create(&config);
  if (!tts) {
    printf("fatal\tcouldn't load the voice model in %s\n", m);
    return 1;
  }
  // Warm up so the first real sentence is fast.
  const SherpaOnnxGeneratedAudio *warm = generate(tts, "Hi.", 0, 1.0f);
  if (warm) destroy_audio(warm);
  printf("ready\t%d\t%d\n", speakers(tts), rate(tts));

  char *line = NULL;
  size_t cap = 0;
  ssize_t len;
  while ((len = getline(&line, &cap, stdin)) > 0) {
    if (line[len - 1] == '\n') line[len - 1] = 0;
    char *fields[7] = {0};
    char *p = line;
    int nf = 0;
    while (p && nf < 7) {
      fields[nf++] = p;
      p = strchr(p, '\t');
      if (p) *p++ = 0;
    }
    if (nf < 6) {
      printf("%s\terr\tbad request\n", fields[0] ? fields[0] : "?");
      continue;
    }
    int sid = atoi(fields[1]);
    float speed = (float)atof(fields[2]);
    if (speed <= 0.1f) speed = 1.0f;
    float loud = (float)atof(fields[4]);
    const char *text = fields[5];
    const char *ph = nf > 6 ? fields[6] : NULL;
    const SherpaOnnxGeneratedAudio *a = NULL;
    if (ph && *ph && generate_cfg) {
      size_t cap_cp = strlen(ph) + 1;
      int32_t *cp = malloc(sizeof(int32_t) * cap_cp);
      int32_t lens[64];
      int32_t n = utf8_codepoints(ph, cp, (int32_t)cap_cp);
      int32_t ns = split_sentences(cp, n, 300, lens, 64);
      SherpaOnnxGenerationConfig cfg;
      memset(&cfg, 0, sizeof(cfg));
      cfg.sid = sid;
      cfg.speed = speed;
      cfg.silence_scale = 0.2f;
      cfg.phoneme_codepoints = cp;
      cfg.phoneme_codepoints_lens = lens;
      cfg.phoneme_codepoints_num_sentences = ns;
      if (n > 0) a = generate_cfg(tts, text, &cfg, NULL, NULL);
      free(cp);
      if (!a || a->n <= 0) {
        if (a) destroy_audio(a);
        a = NULL;
      }
    }
    if (!a) a = generate(tts, text, sid, speed);
    if (!a || a->n <= 0) {
      printf("%s\terr\tno audio\n", fields[0]);
      if (a) destroy_audio(a);
      continue;
    }
    float *pcm = level(a->samples, a->n, loud);
    if (!write_wave(pcm, a->n, a->sample_rate, fields[3])) {
      printf("%s\terr\tcouldn't write %s\n", fields[0], fields[3]);
    } else {
      printf("%s\tok\t%d\t%d\n", fields[0], a->n, a->sample_rate);
    }
    free(pcm);
    destroy_audio(a);
  }
  return 0;
}
