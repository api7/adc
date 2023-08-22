// BEGIN: xz3c4v5b6n7m
package common

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGetContentFromFile(t *testing.T) {
	// create a temporary file
	tmpfile, err := os.CreateTemp("", "example")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(tmpfile.Name()) // clean up

	// write some content to the file
	expectedContent := `name: test
version: "1.0.0"
services:
- name: svc1
  hosts:
  - svc1.example.com
  -`
	if _, err := tmpfile.Write([]byte(expectedContent)); err != nil {
		t.Fatal(err)
	}

	// read the content from the file using GetContentFromFile
	actualContent, err := GetContentFromFile(tmpfile.Name())
	if err != nil {
		t.Fatal(err)
	}

	// assert that the content matches the expected content
	assert.Equal(t, "test", actualContent.Name)
	assert.Equal(t, "1.0.0", actualContent.Version)
}

// END: xz3c4v5b6n7m
