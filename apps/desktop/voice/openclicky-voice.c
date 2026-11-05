// openclicky-voice: a persistent local neural voice (Kokoro via sherpa-onnx's C API).
//
// The model loads once; each request then takes ~0.25× real time on Apple silicon.
//   openclicky-voice <runtime-dir> <model-dir> [threads]
// stdin, one request per line (tab-separated):   id \t sid \t speed \t out.wav \t text
// stdout:  "ready\t<speakers>\t<sample_rate>"  then per request
//          "<id>\tok\t<samples>\t<sample_rate>"  or  "<id>\terr\t<message>"
//
// libsherpa-onnx-c-api.dylib is loaded at runtime with dlopen, so this builds without the
// library present; the vendored header (Apache-2.0, k2-fsa/sherpa-onnx v2.0.1) fixes the ABI.

#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vendor/sherpa-onnx-c-api.h"

typedef const SherpaOnnxOfflineTts *(*create_fn)(const SherpaOnnxOfflineTtsConfig *);
typedef const SherpaOnnxGeneratedAudio *(*generate_fn)(const SherpaOnnxOfflineTts *, const char *, int32_t, float);
typedef void (*destroy_audio_fn)(const SherpaOnnxGeneratedAudio *);
typedef int32_t (*write_wave_fn)(const float *, int32_t, int32_t, const char *);
typedef int32_t (*int_fn)(const SherpaOnnxOfflineTts *);

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
    char *fields[5] = {0};
    char *p = line;
    for (int i = 0; i < 4 && p; i++) {
      fields[i] = p;
      p = strchr(p, '\t');
      if (p) *p++ = 0;
    }
    fields[4] = p;
    if (!fields[4]) {
      printf("%s\terr\tbad request\n", fields[0] ? fields[0] : "?");
      continue;
    }
    int sid = atoi(fields[1]);
    float speed = (float)atof(fields[2]);
    if (speed <= 0.1f) speed = 1.0f;
    const SherpaOnnxGeneratedAudio *a = generate(tts, fields[4], sid, speed);
    if (!a || a->n <= 0) {
      printf("%s\terr\tno audio\n", fields[0]);
      if (a) destroy_audio(a);
      continue;
    }
    if (!write_wave(a->samples, a->n, a->sample_rate, fields[3])) {
      printf("%s\terr\tcouldn't write %s\n", fields[0], fields[3]);
    } else {
      printf("%s\tok\t%d\t%d\n", fields[0], a->n, a->sample_rate);
    }
    destroy_audio(a);
  }
  return 0;
}
