// SPDX-License-Identifier: MPL-2.0
//
// mprotect_scale: compares one batched mprotect() call covering an
// N-page range against N independent single-page mprotect() calls,
// against the same, unmodified CortenMM_rw kernel. This is the
// benchmark reporting the paper's largest measured overhead ratios
// (up to ~40x at N=64), and a realistic pattern for JIT compilers
// toggling W^X permissions or garbage collectors adjusting
// generational protection across a batch of pages.
//
// NOTE on history: an earlier version of this program, run WITHOUT
// the pthread wrapper below, appeared to trigger a heap-corruption
// crash and was initially misdiagnosed as an mprotect-specific kernel
// bug. Re-testing with the pthread wrapper showed mprotect() works
// correctly; the crash was the same guest-runtime startup fault that
// affects any non-pthread-linked static binary on this environment,
// unrelated to mprotect(). This file is the corrected version. See
// the paper's Introduction (contributions list, artifact-
// infrastructure findings) for the full account.
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
	size_t region_size = (size_t)n_pages * PAGE_SIZE;

	// --- Batched (transactional): ONE mprotect() call covering the
	// whole N-page region. ---
	void *base = mmap(NULL, region_size, PROT_READ | PROT_WRITE,
			   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (base == MAP_FAILED) {
		perror("mmap");
		exit(1);
	}
	for (int i = 0; i < n_pages; i++)
		((volatile char *)base)[(size_t)i * PAGE_SIZE] = 1;

	long t0 = rdtsc();
	if (mprotect(base, region_size, PROT_READ) != 0) {
		perror("mprotect batched");
		exit(1);
	}
	long t1 = rdtsc();
	g_batched_cycles = t1 - t0;
	mprotect(base, region_size, PROT_READ | PROT_WRITE);
	munmap(base, region_size);

	// --- Per-page (non-transactional): N separate mprotect() calls,
	// one per individual page. ---
	void *base2 = mmap(NULL, region_size, PROT_READ | PROT_WRITE,
			    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (base2 == MAP_FAILED) {
		perror("mmap2");
		exit(1);
	}
	for (int i = 0; i < n_pages; i++)
		((volatile char *)base2)[(size_t)i * PAGE_SIZE] = 1;

	long t2 = rdtsc();
	for (int i = 0; i < n_pages; i++) {
		void *page = (char *)base2 + (size_t)i * PAGE_SIZE;
		if (mprotect(page, PAGE_SIZE, PROT_READ) != 0) {
			perror("mprotect per-page");
			exit(1);
		}
	}
	long t3 = rdtsc();
	g_per_page_cycles = t3 - t2;
	mprotect(base2, region_size, PROT_READ | PROT_WRITE);
	munmap(base2, region_size);

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

	printf("***MPROTECT_SCALE n_pages=%d***\n", g_n_pages);

	pthread_t t;
	pthread_create(&t, NULL, worker, NULL);
	pthread_join(t, NULL);

	printf("BATCHED_CYCLES %ld\n", g_batched_cycles);
	printf("PER_PAGE_CYCLES %ld\n", g_per_page_cycles);
	printf("PER_PAGE_OVERHEAD_RATIO %.3f\n",
	       (double)g_per_page_cycles / (double)g_batched_cycles);

	return 0;
}
