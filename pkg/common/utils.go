package common

import (
	"fmt"
	"hash/crc32"
	"unsafe"
)

// string2Byte converts string to a byte slice without memory allocation.
func string2Byte(s string) []byte {
	return unsafe.Slice(unsafe.StringData(s), len(s))
}

func GenID(raw string) string {
	if raw == "" {
		return ""
	}
	p := string2Byte(raw)

	res := crc32.ChecksumIEEE(p)
	return fmt.Sprintf("%x", res)
}
