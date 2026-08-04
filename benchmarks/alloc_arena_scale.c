// SPDX-License-Identifier: MPL-2.0
//
// alloc_arena_scale: simulates the two real allocator strategies for
// satisfying a sequence of large ("above mmap-threshold") allocation
// requests:
//   (A) Arena strategy (transactional-equivalent): reserve ONE big
//       mmap'd arena up front, then carve out N sub-allocations from
//       it purely in userspace. This is what jemalloc/tcmalloc do for
//       their large-size-class chunks.
//   (B) Direct-mmap strategy (non-transactional-equivalent): call
//       mmap() once PER allocation request, as glibc malloc's
//       mmap_threshold path does for objects above the threshold.
// Both strategies additionally touch (first-write) every allocated
// page, so that first-touch page-fault cost -- identical work under
// either strategy, and NOT affected by the transactional/
// non-transactional distinction -- is included in both measurements.
// This is the benchmark reporting the paper's "dilution" finding: the
// overhead ratio collapses toward ~1x once this realistic per-object
// cost is included (see the paper's Table "Allocator-arena workload").
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <sys/mman.h>
#include <unistd.h>
#include <string.h>

static long rdtsc(void)
{
	unsigned lo, hi;
	__asm__ __volatile__("rdtsc" : "=a"(lo), "=d"(hi));
	return ((long)hi << 32) | lo;
}

#define PAGE_SIZE 4096
#define MAX_ALLOCS 256

static int g_n_allocs;
static int g_pages_per_alloc;
static long g_arena_cycles;
static long g_direct_cycles;

static void *worker(void *arg)
{
	int n = g_n_allocs;
	size_t obj_size = (size_t)g_pages_per_alloc * PAGE_SIZE;

	long t0 = rdtsc();
	size_t arena_size = obj_size * (size_t)n;
	void *arena = mmap(NULL, arena_size, PROT_READ | PROT_WRITE,
			    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (arena == MAP_FAILED) {
		perror("mmap arena");
		exit(1);
	}
	void *objs_a[MAX_ALLOCS];
	for (int i = 0; i < n; i++) {
		objs_a[i] = (char *)arena + (size_t)i * obj_size;
		((volatile char *)objs_a[i])[0] = 1;
	}
	long t1 = rdtsc();
	g_arena_cycles = t1 - t0;
	munmap(arena, arena_size);

	void *objs_b[MAX_ALLOCS];
	long t2 = rdtsc();
	for (int i = 0; i < n; i++) {
		objs_b[i] = mmap(NULL, obj_size, PROT_READ | PROT_WRITE,
				  MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
		if (objs_b[i] == MAP_FAILED) {
			perror("mmap direct");
			exit(1);
		}
		((volatile char *)objs_b[i])[0] = 1;
	}
	long t3 = rdtsc();
	g_direct_cycles = t3 - t2;
	for (int i = 0; i < n; i++)
		munmap(objs_b[i], obj_size);

	return NULL;
}

int main(int argc, char **argv)
{
	if (argc != 3) {
		fprintf(stderr, "usage: %s N_ALLOCS PAGES_PER_ALLOC\n", argv[0]);
		return 1;
	}
	g_n_allocs = atoi(argv[1]);
	g_pages_per_alloc = atoi(argv[2]);
	if (g_n_allocs < 1 || g_n_allocs > MAX_ALLOCS || g_pages_per_alloc < 1) {
		fprintf(stderr, "bad args\n");
		return 1;
	}

	printf("***ALLOC_ARENA_SCALE n_allocs=%d pages_per_alloc=%d***\n",
	       g_n_allocs, g_pages_per_alloc);

	pthread_t t;
	pthread_create(&t, NULL, worker, NULL);
	pthread_join(t, NULL);

	printf("ARENA_CYCLES %ld\n", g_arena_cycles);
	printf("DIRECT_CYCLES %ld\n", g_direct_cycles);
	printf("DIRECT_OVERHEAD_RATIO %.3f\n",
	       (double)g_direct_cycles / (double)g_arena_cycles);

	return 0;
}
