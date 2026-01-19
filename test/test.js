const greeting = "Hello Web World";

function calculateArea(radius) {
  return Math.PI * radius * radius;
}

const circles = [
  { id: 1, r: 5 },
  { id: 2, r: 10 },
  { id: 3, r: 15 }
];

circles.forEach(c => {
  console.log(`Circle ${c.id}: Area = ${calculateArea(c.r).toFixed(2)}`);
});

// ;;javascript is dynamic
