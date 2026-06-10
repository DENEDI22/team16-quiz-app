import { ComponentFixture, TestBed } from '@angular/core/testing';

import { SinglePlayerQuiz } from './single-player-quiz';

describe('SinglePlayerQuiz', () => {
  let component: SinglePlayerQuiz;
  let fixture: ComponentFixture<SinglePlayerQuiz>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [SinglePlayerQuiz],
    }).compileComponents();

    fixture = TestBed.createComponent(SinglePlayerQuiz);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
