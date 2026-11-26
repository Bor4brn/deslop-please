from logstats.stats import Rate, Tally, Window, percent


def summarize(lines: list[tuple[int, int, int]]) -> str:
    if not lines:
        return "no lines"
    tally = Tally(4)
    window = Window(16)
    rate = Rate(lines[0][0])
    for second, level, latency in lines:
        tally.record(level)
        window.push(latency)
        rate.observe(second)
    busiest = tally.busiest()
    share = percent(tally.count(busiest), tally.total())
    return (
        f"level {busiest} is {share}% of lines; latency {window.mean()} ms mean, "
        f"{window.worst()} ms worst; {rate.per_second()} lines a second"
    )
