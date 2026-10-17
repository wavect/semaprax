"""Exact-value pure-mask LRU; no filesystem or mutable source-read cache."""
from sys import getsizeof

class MaskMemoizer:
    __slots__=("original","cap","entries","hits","misses","evictions","bypasses","maximum_retained")
    def __init__(self,original,cap=4*1024*1024):
        if not 4096<=cap<=4*1024*1024:raise ValueError("cache cap must be4096..4194304bytes")
        self.original=original;self.cap=cap;self.entries=[None]*32
        self.hits=self.misses=self.evictions=self.bypasses=self.maximum_retained=0
    def retained_bytes(self):
        # The existing original function is referenced, not copied/owned here.
        # Count cache instance, fixed slot list, retained tuples/strings and ints.
        return (getsizeof(self)+getsizeof(self.entries)
          +sum(getsizeof(value) for value in [self.cap,self.hits,self.misses,self.evictions,self.bypasses,self.maximum_retained])
          +sum(getsizeof(item)+getsizeof(item[0])+getsizeof(item[1]) for item in self.entries if item is not None))
    def _evict(self):
        for i in range(31,-1,-1):
            if self.entries[i] is not None:self.entries[i]=None;self.evictions+=1;return True
        return False
    def _finish(self):
        while self.retained_bytes()>self.cap:
            if not self._evict():raise AssertionError("fixed cache overhead exceeds cap")
        self.maximum_retained=max(self.maximum_retained,self.retained_bytes())
        # Updating the counter can change its exact integer allocation size.
        while self.retained_bytes()>self.cap:
            if not self._evict():raise AssertionError("fixed cache overhead exceeds cap")
    def __call__(self,text):
        if type(text) is not str:return self.original(text)
        for i,item in enumerate(self.entries):
            if item is not None and item[0]==text:
                self.hits+=1
                for j in range(i,0,-1):self.entries[j]=self.entries[j-1]
                self.entries[0]=item;self._finish();return item[1]
        self.misses+=1
        result=self.original(text) # Exceptions are never cached or replaced.
        if type(result) is not str:self._finish();return result
        item=(text,result)
        charge=getsizeof(item)+getsizeof(text)+getsizeof(result)
        if charge+self.retained_bytes()>self.cap:
            while charge+self.retained_bytes()>self.cap and self._evict():pass
        if charge+self.retained_bytes()>self.cap:
            self.bypasses+=1;self._finish();return result
        if self.entries[-1] is not None:self._evict()
        for j in range(31,0,-1):self.entries[j]=self.entries[j-1]
        self.entries[0]=item;self._finish();return result
    def statistics(self):
        return dict(hits=self.hits,misses=self.misses,evictions=self.evictions,bypasses=self.bypasses,
          cap_bytes=self.cap,retained_bytes=self.retained_bytes(),maximum_retained_bytes=self.maximum_retained,
          slots=32,entry_identity="complete input str equality",file_reads_cached=False,
          accounting="CPython getsizeof retained cache ownership; not RSS or transient computation cap")
