import { ComponentFixture, TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideRouter } from '@angular/router';
import { provideNoopAnimations } from '@angular/platform-browser/animations';

import { SinglePlayerQuiz } from './single-player-quiz';

describe('SinglePlayerQuiz', () => {
  let component: SinglePlayerQuiz;
  let fixture: ComponentFixture<SinglePlayerQuiz>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [SinglePlayerQuiz],
      providers: [provideHttpClient(), provideRouter([]), provideNoopAnimations()],
    }).compileComponents();

    fixture = TestBed.createComponent(SinglePlayerQuiz);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
