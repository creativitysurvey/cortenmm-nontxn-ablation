// SPDX-License-Identifier: MPL-2.0
//
// mmap_batch_scale: compares one batched mmap() call covering an
// N-page range (the transactional calling convention: CortenMM's
// VMAR::map() acquires exactly one cursor_mut() for the whole range)
// against N independent single-page mmap() calls (the
// non-transactional calling convention: N independent
// cursor_mut()-acquire/operate/release cycles), against the same,
// unmodified CortenMM_rw kernel.
//
// Runs inside a dedicated pthread: on this guest environment, static
// binaries that do not link pthread fail at startup with heap
// corruption unrelated to the design axis under test (see the
// paper's artifact-infrastructure findings, Appendix A / Introduction
// contributions list). Linking and spawning a single worker thread is
// a reliable workaround.
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <sys/mman.h>
#include <unistd.h>

static long rdtsc(void)
{
	unsigned lo, hi;
	__asm__ __volatile__("rdtsc" : "=a"(lo), "=d"(hi));
	return ((long)hi << 32) | lo;
}

#define PAGE_SIZE 4096
#define MAX_PAGES 256

static int g_n_pages;
static long g_batched_cycles;
static long g_per_page_cycles;

static void *worker(void *arg)
{
	int n_pages = g_n_pages;

	long t0 = rdtsc();
	void *batched = mmap(NULL, (size_t)n_pages * PAGE_SIZE,
			      PROT_READ | PROT_WRITE,
			      MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	long t1 = rdtsc();
	if (batched == MAP_FAILED) {
		perror("mmap batched");
		exit(1);
	}
	g_batched_cycles = t1 - t0;
	munmap(batched, (size_t)n_pages * PAGE_SIZE);

	void *addrs[MAX_PAGES];
	long t2 = rdtsc();
	for (int i = 0; i < n_pages; i++) {
		addrs[i] = mmap(NULL, PAGE_SIZE, PROT_READ | PROT_WRITE,
				MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
		if (addrs[i] == MAP_FAILED) {
			perror("mmap per-page");
			exit(1);
		}
	}
	long t3 = rdtsc();
	g_per_page_cycles = t3 - t2;
	for (int i = 0; i < n_pages; i++)
		munmap(addrs[i], PAGE_SIZE);

	return NULL;
}

int main(int argc, char **argv)
{
	if (argc != 2) {
		fprintf(stderr, "usage: %s N_PAGES\n", argv[0]);
		return 1;
	}
	g_n_pages = atoi(argv[1]);
	if (g_n_pages < 1 || g_n_pages > MAX_PAGES) {
		fprintf(stderr, "N_PAGES must be in [1, %d]\n", MAX_PAGES);
		return 1;
	}

	printf("***MMAP_BATCH_SCALE n_pages=%d***\n", g_n_pages);

	pthread_t t;
	pthread_create(&t, NULL, worker, NULL);
	pthread_join(t, NULL);

	printf("BATCHED_CYCLES %ld\n", g_batched_cycles);
	printf("PER_PAGE_CYCLES %ld\n", g_per_page_cycles);
	printf("PER_PAGE_OVERHEAD_RATIO %.3f\n",
	       (double)g_per_page_cycles / (double)g_batched_cycles);

	return 0;
}
