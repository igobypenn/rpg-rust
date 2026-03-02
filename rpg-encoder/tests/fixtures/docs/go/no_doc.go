package main

func ProcessData(data []int) int {
	total := 0
	for _, v := range data {
		total += v
	}
	return total
}

type Undocumented struct {
	Value int
}

func (u Undocumented) GetValue() int {
	return u.Value
}
