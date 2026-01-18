def fibonacci(n):
    """
    Generate Fibonacci sequence up to n
    """
    if n <= 0:
        return []
    elif n == 1:
        return [0]
    
    sequence = [0, 1]
    while len(sequence) < n:
        next_val = sequence[-1] + sequence[-2]
        sequence.append(next_val)
        
    return sequence

if __name__ == "__main__":
    n = 10
    print(f"Fibonacci sequence ({n}): {fibonacci(n)}")
    
    # Test AI processor
    # ;;hello python world
