from veloversi import hello_from_bin


def test_smoke_hello_from_bin() -> None:
    assert hello_from_bin() == "Hello from veloversi!"
